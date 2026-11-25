//! Estado de las instalaciones (`installed`, `outdated`) y escaneo de carpetas de agentes (`scan`).

use crate::Context;
use crate::agents::{AgentId, Scope, read_dirs};
use crate::error::Result;
use crate::fsutil;
use crate::hash::tree_hash;
use crate::meta::{SkillMeta, Source, hostname};
use crate::paths::{absolute, project_root};
use crate::registry::{InstallRecord, InstallStatus, Registry};
use crate::skill::validate::{ValidateOptions, validate_dir};
use chrono::Utc;
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct InstallView {
    #[serde(flatten)]
    pub record: InstallRecord,
    pub status: InstallStatus,
    pub status_es: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disk_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bank_hash: Option<String>,
}

struct BankHashes<'a> {
    ctx: &'a Context,
    cache: HashMap<String, Option<String>>,
}

impl BankHashes<'_> {
    fn get(&mut self, name: &str) -> Option<String> {
        if let Some(h) = self.cache.get(name) {
            return h.clone();
        }
        let h = if self.ctx.bank.has(name) { tree_hash(&self.ctx.bank.skill_dir(name)).ok() } else { None };
        self.cache.insert(name.to_string(), h.clone());
        h
    }
}

fn view(rec: &InstallRecord, bank: &mut BankHashes) -> InstallView {
    let p = Path::new(&rec.path);
    let disk = if p.join("SKILL.md").is_file() { tree_hash(p).ok() } else { None };
    let b = bank.get(&rec.skill);
    let status = InstallStatus::compute(disk.as_deref(), &rec.installed_hash, b.as_deref());
    InstallView { record: rec.clone(), status, status_es: status.label_es(), disk_hash: disk, bank_hash: b }
}

/// Instalaciones registradas, opcionalmente filtradas por skill y por proyecto.
pub fn installed(ctx: &Context, skill: Option<&str>, project: Option<&Path>) -> Result<Vec<InstallView>> {
    let reg = Registry::load(&ctx.paths.registry_file())?;
    let root = project.map(|p| project_root(p).display().to_string());
    let mut bank = BankHashes { ctx, cache: HashMap::new() };
    let mut out: Vec<InstallView> = reg
        .installs
        .iter()
        .filter(|r| skill.is_none_or(|s| r.skill == s))
        .filter(|r| root.as_ref().is_none_or(|root| r.project_root.as_deref() == Some(root.as_str())))
        .map(|r| view(r, &mut bank))
        .collect();
    out.sort_by(|a, b| a.record.skill.cmp(&b.record.skill).then(a.record.path.cmp(&b.record.path)));
    Ok(out)
}

#[derive(Debug, Clone, Serialize)]
pub struct OutdatedOutcome {
    /// Instalaciones que no están al día.
    pub items: Vec<InstallView>,
    /// Rutas actualizadas con `--update`.
    pub updated: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registry_warning: Option<String>,
}

/// Instalaciones que no están al día. Con `update`, actualiza las `desactualizadas`
/// (las que no tienen cambios locales); las demás necesitan una decisión del usuario.
pub fn outdated(ctx: &Context, project: Option<&Path>, update: bool) -> Result<OutdatedOutcome> {
    let items: Vec<InstallView> =
        installed(ctx, None, project)?.into_iter().filter(|v| v.status != InstallStatus::UpToDate).collect();
    let mut out = OutdatedOutcome { items, updated: vec![], registry_warning: None };
    if !update || !out.items.iter().any(|v| v.status == InstallStatus::Outdated) {
        return Ok(out);
    }
    let _lock = ctx.lock_exclusive()?;
    let mut reg = Registry::load(&ctx.paths.registry_file())?;
    for v in out.items.iter_mut().filter(|v| v.status == InstallStatus::Outdated) {
        let target = PathBuf::from(&v.record.path);
        fsutil::replace_dir_with_copy(&ctx.bank.skill_dir(&v.record.skill), &target)?;
        fsutil::set_executables(&target, &ctx.bank.meta(&v.record.skill).executables)?;
        let mut rec = v.record.clone();
        rec.installed_hash = v.bank_hash.clone().unwrap_or_default();
        rec.installed_at = Utc::now();
        rec.origin = "pull".into();
        reg.upsert(rec);
        v.status = InstallStatus::UpToDate;
        v.status_es = v.status.label_es();
        out.updated.push(v.record.path.clone());
    }
    if let Err(e) = reg.save(&ctx.paths.registry_file()) {
        out.registry_warning = Some(format!("REGISTRY_NOT_WRITTEN: {e}"));
    }
    Ok(out)
}

// ───────────────────────────── scan ─────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct ScanOptions {
    /// Escanea las carpetas globales de los agentes en vez de un proyecto.
    pub global: bool,
    pub dir: Option<PathBuf>,
    pub cwd: PathBuf,
    /// Registra como instaladas las copias idénticas al banco.
    pub register: bool,
    /// Sube al banco las skills que no están en él (y las registra).
    pub import: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScanEntry {
    pub name: String,
    pub path: String,
    /// Destino real si `path` es (o pasa por) un enlace simbólico.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub real_path: Option<String>,
    /// Otras rutas que apuntan a la misma carpeta (enlaces).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub also_at: Vec<String>,
    pub scope: Scope,
    pub agents: Vec<AgentId>,
    /// Skill sincronizada desde claude.ai (solo lectura).
    pub synced: bool,
    /// same | different | absent | invalid
    pub status: String,
    pub registered: bool,
    /// registered | imported (acción hecha en este escaneo)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScanOutcome {
    pub scope: Scope,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_root: Option<String>,
    pub dirs: Vec<String>,
    pub entries: Vec<ScanEntry>,
    pub registered: usize,
    pub imported: usize,
}

/// Skills directas dentro de una carpeta de skills (más las de `synced/`).
fn skills_in(dir: &Path) -> Vec<(PathBuf, bool)> {
    let mut out = Vec::new();
    let Ok(rd) = fs::read_dir(dir) else { return out };
    let mut entries: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if name.starts_with('.') {
            continue;
        }
        if p.join("SKILL.md").is_file() {
            out.push((p, false));
        } else if name.eq_ignore_ascii_case("synced") && p.is_dir() {
            out.extend(skills_in(&p).into_iter().map(|(s, _)| (s, true)));
        }
    }
    out
}

/// Busca skills en las carpetas de los agentes y las compara con el banco.
pub fn scan(ctx: &Context, opts: &ScanOptions) -> Result<ScanOutcome> {
    let (scope, root) = if opts.global {
        (Scope::Global, ctx.paths.home.clone())
    } else {
        (Scope::Project, project_root(opts.dir.as_deref().unwrap_or(&opts.cwd)))
    };
    let mut dirs: Vec<(PathBuf, Vec<AgentId>)> = Vec::new();
    for a in AgentId::ALL {
        for d in read_dirs(&ctx.config.agent(a), scope, &root, &ctx.paths) {
            match dirs.iter_mut().find(|(x, _)| *x == d) {
                Some((_, agents)) => agents.push(a),
                None => dirs.push((d, vec![a])),
            }
        }
    }
    let reg = Registry::load(&ctx.paths.registry_file()).unwrap_or_default();
    let mut bank = BankHashes { ctx, cache: HashMap::new() };
    let mut entries: Vec<ScanEntry> = Vec::new();
    let mut by_real: HashMap<PathBuf, usize> = HashMap::new();
    let mut hashes: Vec<Option<String>> = Vec::new();

    for (d, agents) in &dirs {
        for (skill, synced) in skills_in(d) {
            // La guía del propio banco no es una skill del usuario.
            if skill.file_name().is_some_and(|n| n == crate::guide::GUIDE_NAME) {
                continue;
            }
            let path = absolute(&skill);
            let real = fs::canonicalize(&skill).unwrap_or_else(|_| path.clone());
            if let Some(i) = by_real.get(&real) {
                let e = &mut entries[*i];
                if path == real && e.real_path.is_some() {
                    // Preferimos la copia real como ruta principal y dejamos el enlace como secundaria.
                    e.also_at.push(std::mem::replace(&mut e.path, path.display().to_string()));
                    e.real_path = None;
                    e.registered = reg.find_by_path(&e.path).is_some();
                } else {
                    e.also_at.push(path.display().to_string());
                }
                for a in agents {
                    if !e.agents.contains(a) {
                        e.agents.push(*a);
                    }
                }
                continue;
            }
            let report = validate_dir(&skill, &ValidateOptions { dir_mismatch_is_warning: true, ..Default::default() });
            let name = report
                .name
                .clone()
                .unwrap_or_else(|| skill.file_name().unwrap_or_default().to_string_lossy().into_owned());
            let disk = tree_hash(&skill).ok();
            let status = if report.has_errors() {
                "invalid"
            } else {
                match bank.get(&name) {
                    None => "absent",
                    Some(b) if Some(&b) == disk.as_ref() => "same",
                    Some(_) => "different",
                }
            };
            by_real.insert(real.clone(), entries.len());
            hashes.push(disk);
            entries.push(ScanEntry {
                name,
                path: path.display().to_string(),
                real_path: (real != path).then(|| real.display().to_string()),
                also_at: vec![],
                scope,
                agents: agents.clone(),
                synced,
                status: status.into(),
                registered: reg.find_by_path(&path.display().to_string()).is_some(),
                action: None,
            });
        }
    }

    let mut out = ScanOutcome {
        scope,
        project_root: (scope == Scope::Project).then(|| root.display().to_string()),
        dirs: dirs.iter().map(|(d, _)| d.display().to_string()).collect(),
        entries,
        registered: 0,
        imported: 0,
    };
    if !(opts.register || opts.import) {
        return Ok(out);
    }

    let _lock = ctx.lock_exclusive()?;
    let mut reg = Registry::load(&ctx.paths.registry_file()).unwrap_or_default();
    for (i, e) in out.entries.iter_mut().enumerate() {
        let Some(h) = hashes[i].clone() else { continue };
        // Solo se registran copias reales (no enlaces) y que no vengan de claude.ai.
        let registrable = e.real_path.is_none() && !e.synced && !fsutil::is_symlink(Path::new(&e.path));
        // Dos copias con el mismo nombre: la segunda se compara con lo que ya se importó.
        if opts.import && e.status == "absent" && ctx.bank.has(&e.name) {
            let b = tree_hash(&ctx.bank.skill_dir(&e.name)).ok();
            e.status = if b.as_deref() == Some(h.as_str()) { "same" } else { "different" }.into();
        }
        if opts.import && e.status == "absent" {
            let files = ctx.bank.store(&e.name, Path::new(&e.path), &h)?;
            let mut meta = SkillMeta::new(&e.name);
            meta.content_hash = h.clone();
            meta.executables = files.iter().filter(|f| f.executable).map(|f| f.rel.clone()).collect();
            meta.source =
                Some(Source { kind: "scan".into(), path: Some(e.path.clone()), agent: None, host: hostname() });
            ctx.bank.save_meta(&meta)?;
            e.status = "same".into();
            e.action = Some("imported".into());
            out.imported += 1;
        }
        if (opts.register || e.action.is_some()) && e.status == "same" && registrable && !e.registered {
            reg.upsert(InstallRecord {
                id: uuid::Uuid::new_v4().to_string(),
                skill: e.name.clone(),
                scope,
                agents: e.agents.clone(),
                project_root: out.project_root.clone(),
                path: e.path.clone(),
                installed_hash: h,
                installed_at: Utc::now(),
                origin: "scan".into(),
            });
            e.registered = true;
            if e.action.is_none() {
                e.action = Some("registered".into());
            }
            out.registered += 1;
        }
    }
    if out.imported > 0 {
        crate::sync::mark_dirty(&ctx.paths.sync_state_file());
    }
    reg.save(&ctx.paths.registry_file())?;
    Ok(out)
}
