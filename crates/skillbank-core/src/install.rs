//! Subir (push) skills al banco y traerlas (pull) a proyectos o carpetas globales de agentes.

use crate::Context;
use crate::agents::{
    AgentId, AgentRequest, AgentSource, Destination, Scope, plan_destinations, read_dirs, resolve_agents,
};
use crate::diff::{TreeDiff, diff_trees};
use crate::error::{CoreError, ErrorKind, Result};
use crate::fsutil;
use crate::hash::tree_hash;
use crate::meta::{SkillMeta, Source, hostname, normalize_tags};
use crate::paths::{absolute, project_root};
use crate::registry::{InstallRecord, Registry};
use crate::skill::resolve_skill_dir;
use crate::skill::validate::{Diagnostic, Severity, ValidateOptions, validate_dir};
use chrono::Utc;
use serde::Serialize;
use serde_json::json;
use std::path::{Path, PathBuf};

/// Máximo de líneas del parche de SKILL.md en un conflicto (salvo `--diff`).
pub const CONFLICT_PATCH_LINES: usize = 40;

// ───────────────────────────── push ─────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct PushOptions {
    pub force: bool,
    /// Etiquetas a asignar (reemplazan las existentes salvo `add_tags`).
    pub tags: Option<Vec<String>>,
    pub add_tags: bool,
    /// Permite subir una skill con errores de validación (salvo errores de `name`).
    pub allow_invalid: bool,
    pub dry_run: bool,
    /// Parche completo de SKILL.md en el conflicto.
    pub full_diff: bool,
    pub cwd: PathBuf,
    /// Agente que hace la subida (solo informativo).
    pub agent: Option<AgentId>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PushStatus {
    Created,
    Updated,
    Unchanged,
}

/// Relación entre la copia local y la del banco cuando difieren.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Relation {
    /// El banco no cambió desde que se instaló aquí: solo se añadirían tus cambios locales.
    FastForward,
    /// La copia local no cambió, pero el banco sí: subir revertiría el banco a una versión anterior.
    BankNewer,
    /// Cambiaron ambas.
    Diverged,
    /// No hay registro de instalación: no se sabe cuál es más nueva.
    Unrelated,
}

impl Relation {
    pub fn explain_es(self) -> &'static str {
        match self {
            Relation::FastForward => {
                "El banco no ha cambiado desde que se instaló aquí; solo se añadirían los cambios locales."
            }
            Relation::BankNewer => {
                "La copia local no ha cambiado, pero el banco tiene una versión más nueva: sobrescribir la revertiría."
            }
            Relation::Diverged => "Tanto la copia local como el banco cambiaron desde la instalación.",
            Relation::Unrelated => "No hay registro de dónde salió esta copia; no se sabe cuál es más nueva.",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PushOutcome {
    pub name: String,
    pub status: PushStatus,
    pub dry_run: bool,
    pub source: String,
    pub bank_path: String,
    pub hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_hash: Option<String>,
    pub tags: Vec<String>,
    pub warnings: Vec<Diagnostic>,
    pub registered: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registry_warning: Option<String>,
}

/// Lugar de instalación reconocido de una carpeta de skill (para el registro).
#[derive(Debug, Clone)]
pub struct Location {
    pub scope: Scope,
    pub agents: Vec<AgentId>,
    pub project_root: Option<PathBuf>,
}

/// ¿Está esta carpeta de skill dentro de una carpeta de skills de algún agente?
pub fn location_of(ctx: &Context, skill_dir: &Path) -> Option<Location> {
    let skill_dir = absolute(skill_dir);
    let parent = skill_dir.parent()?.to_path_buf();
    let mut agents: Vec<AgentId> = AgentId::ALL
        .into_iter()
        .filter(|a| read_dirs(&ctx.config.agent(*a), Scope::Global, &ctx.paths.home, &ctx.paths).contains(&parent))
        .collect();
    if !agents.is_empty() {
        return Some(Location { scope: Scope::Global, agents, project_root: None });
    }
    let root = project_root(&skill_dir);
    agents = AgentId::ALL
        .into_iter()
        .filter(|a| read_dirs(&ctx.config.agent(*a), Scope::Project, &root, &ctx.paths).contains(&parent))
        .collect();
    if agents.is_empty() {
        return None;
    }
    Some(Location { scope: Scope::Project, agents, project_root: Some(root) })
}

/// Carpetas de skills (de proyecto y globales) donde buscar una skill por nombre.
fn candidate_dirs(ctx: &Context, cwd: &Path) -> Vec<PathBuf> {
    let root = project_root(cwd);
    let mut dirs = Vec::new();
    for scope in [Scope::Project, Scope::Global] {
        for a in AgentId::ALL {
            for d in read_dirs(&ctx.config.agent(a), scope, &root, &ctx.paths) {
                if !dirs.contains(&d) {
                    dirs.push(d);
                }
            }
        }
    }
    dirs
}

/// Resuelve el argumento de `push`: ruta (carpeta o SKILL.md) o nombre buscado en las carpetas de agentes.
pub fn resolve_push_source(ctx: &Context, arg: &str, cwd: &Path) -> Result<PathBuf> {
    let as_path = if Path::new(arg).is_absolute() { PathBuf::from(arg) } else { cwd.join(arg) };
    if let Some(dir) = resolve_skill_dir(&as_path) {
        return Ok(absolute(&dir));
    }
    if as_path.exists() {
        return Err(CoreError::new(
            ErrorKind::Invalid,
            format!("{} no es una skill: falta SKILL.md.", as_path.display()),
        )
        .with_hint("Indica la carpeta de la skill (la que contiene SKILL.md)."));
    }
    if arg.contains('/') {
        return Err(CoreError::not_found(format!("No existe la ruta {}", as_path.display())));
    }
    // Buscar por nombre en las carpetas de skills de los agentes.
    let mut found: Vec<(PathBuf, PathBuf)> = Vec::new(); // (ruta, ruta canónica)
    for d in candidate_dirs(ctx, cwd) {
        let p = d.join(arg);
        if p.join("SKILL.md").is_file() {
            let canon = std::fs::canonicalize(&p).unwrap_or_else(|_| p.clone());
            if !found.iter().any(|(_, c)| *c == canon) {
                found.push((p, canon));
            }
        }
    }
    match found.len() {
        0 => Err(CoreError::not_found(format!(
            "No encontré una skill llamada '{arg}' en este proyecto ni en las carpetas globales de los agentes."
        ))
        .with_hint("Indica la ruta de la carpeta de la skill, p. ej. `skillbank push .claude/skills/mi-skill`.")),
        1 => Ok(found.remove(0).0),
        _ => {
            let hashes: Vec<String> = found.iter().map(|(p, _)| tree_hash(p).unwrap_or_default()).collect();
            if hashes.windows(2).all(|w| w[0] == w[1]) {
                return Ok(found.remove(0).0);
            }
            let paths: Vec<String> = found.iter().map(|(p, _)| p.display().to_string()).collect();
            Err(CoreError::new(
                ErrorKind::Ambiguous,
                format!("Hay {} copias distintas de '{arg}'; indica cuál subir con su ruta.", paths.len()),
            )
            .with_data(json!({ "candidates": paths })))
        }
    }
}

fn compute_relation(reg: &Registry, src: &Path, bank_hash: &str, src_hash: &str) -> Relation {
    match reg.find_by_path(&src.display().to_string()) {
        Some(r) if r.installed_hash == bank_hash => Relation::FastForward,
        Some(r) if r.installed_hash == src_hash => Relation::BankNewer,
        Some(_) => Relation::Diverged,
        None => Relation::Unrelated,
    }
}

/// Sube una skill al banco.
pub fn push(ctx: &Context, arg: &str, opts: &PushOptions) -> Result<PushOutcome> {
    let src = resolve_push_source(ctx, arg, &opts.cwd)?;
    let report = validate_dir(&src, &ValidateOptions { dir_mismatch_is_warning: true, ..Default::default() });
    let name_errors = report.errors().any(|d| {
        d.code.starts_with("E_NAME")
            || d.code.starts_with("E_FM")
            || d.code == "E_SKILLMD_MISSING"
            || d.code == "E_UTF8"
    });
    if report.has_errors() && (name_errors || !opts.allow_invalid) {
        let n = report.count(Severity::Error);
        return Err(CoreError::new(
            ErrorKind::Invalid,
            format!("La skill tiene {n} error(es) de validación; no se subió."),
        )
        .with_hint("Corrige los errores (ver `diagnostics`) y vuelve a intentarlo.")
        .with_data(json!({ "source": src.display().to_string(), "diagnostics": report.diagnostics })));
    }
    let name = report.name.clone().ok_or_else(|| CoreError::new(ErrorKind::Invalid, "La skill no tiene `name`."))?;
    let warnings: Vec<Diagnostic> =
        report.diagnostics.iter().filter(|d| d.severity != Severity::Info).cloned().collect();
    let src_hash = tree_hash(&src)?;

    let _lock = if opts.dry_run { None } else { Some(ctx.lock_exclusive()?) };
    let bank = &ctx.bank;
    let mut reg = Registry::load(&ctx.paths.registry_file()).unwrap_or_default();
    let bank_dir = bank.skill_dir(&name);
    let existed = bank.has(&name);
    let mut previous_hash = None;
    let mut status = PushStatus::Created;

    if existed {
        let b_hash = tree_hash(&bank_dir)?;
        if b_hash == src_hash {
            status = PushStatus::Unchanged;
        } else {
            previous_hash = Some(b_hash.clone());
            status = PushStatus::Updated;
            if !opts.force {
                let relation = compute_relation(&reg, &src, &b_hash, &src_hash);
                let max = if opts.full_diff { None } else { Some(CONFLICT_PATCH_LINES) };
                let diff = diff_trees(&bank_dir, &src, "banco", "local", max)?;
                return Err(conflict_error(&name, &src, &b_hash, &src_hash, relation, &diff));
            }
        }
    }

    let mut meta = if existed { bank.meta(&name) } else { SkillMeta::new(&name) };
    if let Some(t) = &opts.tags {
        meta.tags = if opts.add_tags {
            normalize_tags(meta.tags.iter().cloned().chain(t.iter().cloned()))
        } else {
            normalize_tags(t)
        };
    }

    let mut outcome = PushOutcome {
        name: name.clone(),
        status,
        dry_run: opts.dry_run,
        source: src.display().to_string(),
        bank_path: bank_dir.display().to_string(),
        hash: src_hash.clone(),
        previous_hash,
        tags: meta.tags.clone(),
        warnings,
        registered: false,
        registry_warning: None,
    };
    if opts.dry_run {
        return Ok(outcome);
    }

    if status != PushStatus::Unchanged {
        let files = bank.store(&name, &src, &src_hash)?;
        meta.updated_at = Utc::now();
        meta.content_hash = src_hash.clone();
        meta.executables = files.iter().filter(|f| f.executable).map(|f| f.rel.clone()).collect();
        meta.source = Some(Source {
            kind: "push".into(),
            path: Some(src.display().to_string()),
            agent: opts.agent.map(|a| a.key().to_string()),
            host: hostname(),
        });
        crate::sync::mark_dirty(&ctx.paths.sync_state_file());
    } else if meta.content_hash.is_empty() {
        meta.content_hash = src_hash.clone();
    }
    if status != PushStatus::Unchanged || opts.tags.is_some() || !ctx.bank.meta_path(&name).exists() {
        bank.save_meta(&meta)?;
    }
    bank.purge_trash();

    if let Some(loc) = location_of(ctx, &src) {
        reg.upsert(InstallRecord {
            id: uuid::Uuid::new_v4().to_string(),
            skill: name.clone(),
            scope: loc.scope,
            agents: loc.agents,
            project_root: loc.project_root.map(|p| p.display().to_string()),
            path: src.display().to_string(),
            installed_hash: src_hash,
            installed_at: Utc::now(),
            origin: "push".into(),
        });
        match reg.save(&ctx.paths.registry_file()) {
            Ok(()) => outcome.registered = true,
            Err(e) => outcome.registry_warning = Some(format!("REGISTRY_NOT_WRITTEN: {e}")),
        }
    }
    Ok(outcome)
}

fn conflict_error(
    name: &str,
    src: &Path,
    bank_hash: &str,
    src_hash: &str,
    relation: Relation,
    diff: &TreeDiff,
) -> CoreError {
    CoreError::new(
        ErrorKind::Conflict,
        format!("La skill '{name}' ya existe en el banco y es distinta. Cambios: {}.", diff.summary_es()),
    )
    .with_hint(format!(
        "{} Muestra el resumen al usuario y, solo si acepta sobrescribir, repite con --force.",
        relation.explain_es()
    ))
    .with_data(json!({
        "name": name,
        "source": src.display().to_string(),
        "relation": relation,
        "bank_hash": bank_hash,
        "source_hash": src_hash,
        "diff": diff,
    }))
}

// ───────────────────────────── pull ─────────────────────────────

#[derive(Debug, Clone)]
pub struct PullOptions {
    pub agents: AgentRequest,
    pub scope: Scope,
    /// Carpeta del proyecto (por defecto, el cwd). Se sube hasta el `.git` más cercano.
    pub project: Option<PathBuf>,
    pub cwd: PathBuf,
    pub force: bool,
    pub dry_run: bool,
    pub dedupe: bool,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PullAction {
    Installed,
    Updated,
    Unchanged,
    Conflict,
}

#[derive(Debug, Clone, Serialize)]
pub struct PullItem {
    pub skill: String,
    /// Carpeta final de la skill instalada.
    pub path: String,
    pub agents: Vec<AgentId>,
    pub action: PullAction,
    /// symlink | modified | unregistered | not_a_dir | forced
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_hash: Option<String>,
    pub bank_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff: Option<TreeDiff>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PullOutcome {
    pub scope: Scope,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_root: Option<String>,
    pub agents: Vec<AgentId>,
    pub agent_source: AgentSource,
    pub destinations: Vec<Destination>,
    pub items: Vec<PullItem>,
    pub warnings: Vec<Diagnostic>,
    pub dry_run: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registry_warning: Option<String>,
}

fn reason_es(reason: &str) -> &'static str {
    match reason {
        "symlink" => "el destino es un enlace simbólico",
        "modified" => "la copia local tiene cambios que no están en el banco",
        "unregistered" => "ya existe una copia distinta que no se instaló desde el banco",
        "not_a_dir" => "existe un archivo con ese nombre",
        _ => "conflicto",
    }
}

/// Trae una o varias skills del banco.
pub fn pull(ctx: &Context, names: &[String], opts: &PullOptions) -> Result<PullOutcome> {
    if names.is_empty() {
        return Err(CoreError::usage("Indica al menos una skill."));
    }
    let root = match opts.scope {
        Scope::Project => project_root(opts.project.as_deref().unwrap_or(&opts.cwd)),
        Scope::Global => ctx.paths.home.clone(),
    };
    let (agents, agent_source) =
        resolve_agents(&opts.agents, (opts.scope == Scope::Project).then_some(root.as_path()), &ctx.config);
    let dests = plan_destinations(&agents, opts.scope, &root, &ctx.config, &ctx.paths, opts.dedupe);

    for n in names {
        ctx.bank.require(n)?;
    }
    let _lock = if opts.dry_run { None } else { Some(ctx.lock_exclusive()?) };
    let mut reg = Registry::load(&ctx.paths.registry_file()).unwrap_or_default();

    let mut items = Vec::new();
    let mut warnings = Vec::new();
    for name in names {
        let bank_dir = ctx.bank.skill_dir(name);
        let b_hash = tree_hash(&bank_dir)?;
        let compat = validate_dir(&bank_dir, &ValidateOptions { target_agents: agents.clone(), ..Default::default() });
        warnings.extend(compat.diagnostics.into_iter().filter(|d| d.code == "W_CC_ONLY_TARGET"));
        for d in &dests {
            let target = d.dir.join(name);
            let target_s = target.display().to_string();
            let mut item = PullItem {
                skill: name.clone(),
                path: target_s.clone(),
                agents: d.agents.clone(),
                action: PullAction::Installed,
                reason: None,
                local_hash: None,
                bank_hash: b_hash.clone(),
                diff: None,
            };
            if fsutil::is_symlink(&target) {
                item.action = PullAction::Conflict;
                item.reason = Some("symlink".into());
            } else if target.exists() {
                if !target.is_dir() {
                    item.action = PullAction::Conflict;
                    item.reason = Some("not_a_dir".into());
                } else {
                    let local = tree_hash(&target)?;
                    item.local_hash = Some(local.clone());
                    if local == b_hash {
                        item.action = PullAction::Unchanged;
                    } else {
                        match reg.find_by_path(&target_s) {
                            Some(r) if r.installed_hash == local => item.action = PullAction::Updated,
                            Some(_) => {
                                item.action = PullAction::Conflict;
                                item.reason = Some("modified".into());
                            }
                            None => {
                                item.action = PullAction::Conflict;
                                item.reason = Some("unregistered".into());
                            }
                        }
                        if item.action == PullAction::Conflict {
                            item.diff =
                                diff_trees(&bank_dir, &target, "banco", "local", Some(CONFLICT_PATCH_LINES)).ok();
                        }
                    }
                }
            }
            items.push(item);
        }
    }

    let mut outcome = PullOutcome {
        scope: opts.scope,
        project_root: (opts.scope == Scope::Project).then(|| root.display().to_string()),
        agents: agents.clone(),
        agent_source,
        destinations: dests.clone(),
        items,
        warnings,
        dry_run: opts.dry_run,
        registry_warning: None,
    };

    let conflicts: Vec<&PullItem> = outcome.items.iter().filter(|i| i.action == PullAction::Conflict).collect();
    if !conflicts.is_empty() && !opts.force {
        let detail = conflicts
            .iter()
            .map(|c| format!("{} ({})", c.path, reason_es(c.reason.as_deref().unwrap_or(""))))
            .collect::<Vec<_>>()
            .join("; ");
        return Err(CoreError::new(ErrorKind::Conflict, format!("No se instaló nada. Conflicto en: {detail}."))
            .with_hint(
                "Si la copia local tiene cambios valiosos, ofrece subirlos primero con `skillbank push`. \
                 Solo con un sí explícito del usuario, repite con --force para sobrescribir.",
            )
            .with_data(serde_json::to_value(&outcome)?));
    }
    if opts.dry_run {
        return Ok(outcome);
    }

    for item in outcome.items.iter_mut() {
        if item.action == PullAction::Conflict {
            item.action = PullAction::Updated;
            item.reason = Some("forced".into());
        }
        let target = PathBuf::from(&item.path);
        if item.action != PullAction::Unchanged {
            let bank_dir = ctx.bank.skill_dir(&item.skill);
            fsutil::replace_dir_with_copy(&bank_dir, &target)?;
            let meta = ctx.bank.meta(&item.skill);
            fsutil::set_executables(&target, &meta.executables)?;
        }
        reg.upsert(InstallRecord {
            id: uuid::Uuid::new_v4().to_string(),
            skill: item.skill.clone(),
            scope: opts.scope,
            agents: item.agents.clone(),
            project_root: outcome.project_root.clone(),
            path: item.path.clone(),
            installed_hash: item.bank_hash.clone(),
            installed_at: Utc::now(),
            origin: "pull".into(),
        });
        item.diff = None;
    }
    if let Err(e) = reg.save(&ctx.paths.registry_file()) {
        outcome.registry_warning = Some(format!("REGISTRY_NOT_WRITTEN: {e}"));
    }
    Ok(outcome)
}

// ───────────────────────────── remove ─────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct RemoveOutcome {
    pub name: String,
    pub trash_path: String,
    pub installs_left: usize,
}

/// Elimina una skill del banco (va a la papelera local durante 7 días).
pub fn remove(ctx: &Context, name: &str) -> Result<RemoveOutcome> {
    ctx.bank.require(name)?;
    let _lock = ctx.lock_exclusive()?;
    let trash = ctx.bank.remove(name)?;
    crate::sync::mark_dirty(&ctx.paths.sync_state_file());
    let reg = Registry::load(&ctx.paths.registry_file()).unwrap_or_default();
    Ok(RemoveOutcome {
        name: name.to_string(),
        trash_path: trash.display().to_string(),
        installs_left: reg.for_skill(name).len(),
    })
}
