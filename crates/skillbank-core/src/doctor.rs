//! Diagnóstico de la instalación (`skillbank doctor`). Con `fix`, aplica solo arreglos seguros.

use crate::Context;
use crate::agents::AgentId;
use crate::guide;
use crate::hash::tree_hash;
use crate::lock::{BankLock, LockMode};
use crate::meta::SkillMeta;
use crate::registry::Registry;
use crate::skill::validate::{ValidateOptions, validate_dir};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Versión mínima de rclone para `bisync` (Fase 3).
pub const RCLONE_MIN: (u32, u32) = (1, 66);

#[derive(Debug, Clone, Serialize)]
pub struct Check {
    pub id: String,
    /// ok | info | warn | error
    pub status: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    pub fixable: bool,
    pub fixed: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct DoctorReport {
    pub checks: Vec<Check>,
    pub errors: usize,
    pub warnings: usize,
    pub fixed: usize,
}

struct Checks(Vec<Check>);

impl Checks {
    fn add(&mut self, id: &str, status: &str, message: impl Into<String>) -> &mut Check {
        self.0.push(Check {
            id: id.into(),
            status: status.into(),
            message: message.into(),
            hint: None,
            fixable: false,
            fixed: false,
        });
        self.0.last_mut().unwrap()
    }
}

impl Check {
    fn hint(&mut self, h: impl Into<String>) -> &mut Check {
        self.hint = Some(h.into());
        self
    }
    fn fix(&mut self, fixed: bool) -> &mut Check {
        self.fixable = true;
        self.fixed = fixed;
        if fixed {
            self.status = "ok".into();
        }
        self
    }
}

/// Ejecutables con ese nombre en el PATH (en orden).
pub fn which_all(bin: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Some(path) = std::env::var_os("PATH") else { return out };
    for dir in std::env::split_paths(&path) {
        let p = dir.join(bin);
        if p.is_file() {
            let canon = fs::canonicalize(&p).unwrap_or(p.clone());
            if !out.iter().any(|x: &PathBuf| fs::canonicalize(x).unwrap_or(x.clone()) == canon) {
                out.push(p);
            }
        }
    }
    out
}

/// Versión de rclone (mayor, menor) y texto, si está instalado.
pub fn rclone_version() -> Option<((u32, u32), String)> {
    let out = std::process::Command::new("rclone").arg("version").output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let first = text.lines().next()?.to_string();
    let v = first.split_whitespace().find(|w| w.starts_with('v'))?.trim_start_matches('v');
    let mut parts = v.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.split('-').next()?.parse().ok()?;
    Some(((major, minor), first))
}

fn writable(dir: &Path) -> bool {
    let probe = dir.join(format!(".skillbank-probe-{}", uuid::Uuid::new_v4().simple()));
    let ok = fs::write(&probe, b"x").is_ok();
    let _ = fs::remove_file(&probe);
    ok
}

pub fn doctor(ctx: &Context, fix: bool, bin_path: &str) -> DoctorReport {
    let mut c = Checks(Vec::new());
    let home = &ctx.paths.home;
    let pretty = |p: &Path| crate::paths::pretty(p, home);

    // Configuración
    if ctx.paths.config_file.exists() {
        c.add("config", "ok", format!("Configuración leída de {}.", pretty(&ctx.paths.config_file)));
    } else {
        c.add("config", "info", "Sin config.toml: se usan los valores por defecto.");
    }

    // Banco
    let bank = &ctx.bank;
    if !bank.is_initialized() {
        c.add("bank", "info", format!("El banco aún no existe ({}); se crea con el primer push.", pretty(&bank.root)));
    } else if !writable(&bank.root) {
        c.add("bank", "error", format!("No se puede escribir en el banco ({}).", pretty(&bank.root)))
            .hint("Revisa los permisos de la carpeta.");
    } else {
        let n = bank.names().map(|v| v.len()).unwrap_or(0);
        c.add("bank", "ok", format!("Banco en {} con {n} skill(s).", pretty(&bank.root)));
        match BankLock::acquire(&bank.lock_path(), LockMode::Shared, Duration::from_millis(0)) {
            Ok(_) => c.add("lock", "ok", "El banco no está bloqueado."),
            Err(_) => c.add("lock", "warn", "El banco está bloqueado por otra operación en curso."),
        };
    }

    // CLI en el PATH
    let bins = which_all("skillbank");
    match bins.len() {
        0 => {
            c.add("cli", "warn", "`skillbank` no está en el PATH: los agentes no lo encontrarán.")
                .hint("Ejecuta scripts/install-cli.sh (instala en ~/.local/bin).");
        }
        1 => {
            c.add("cli", "ok", format!("CLI en el PATH: {}.", pretty(&bins[0])));
        }
        _ => {
            let list = bins.iter().map(|b| pretty(b)).collect::<Vec<_>>().join(", ");
            c.add("cli", "warn", format!("Hay varios `skillbank` en el PATH: {list}. Se usa el primero."))
                .hint("Borra los que sobren para no usar una versión vieja.");
        }
    }

    // Agentes y guía
    let present: Vec<AgentId> = guide::present_agents(ctx);
    c.add(
        "agents",
        "info",
        if present.is_empty() {
            "No se detectó ningún agente instalado.".to_string()
        } else {
            format!("Agentes detectados: {}.", present.iter().map(|a| a.display()).collect::<Vec<_>>().join(", "))
        },
    );
    let needs_guide: Vec<AgentId> = present
        .iter()
        .copied()
        .filter(|a| matches!(guide::state_for_agent(ctx, *a).as_str(), "missing" | "outdated"))
        .collect();
    let foreign: Vec<AgentId> =
        present.iter().copied().filter(|a| guide::state_for_agent(ctx, *a) == "foreign").collect();
    if needs_guide.is_empty() && foreign.is_empty() {
        if !present.is_empty() {
            c.add("guide", "ok", format!("Skill guía al día (versión {}).", guide::GUIDE_VERSION));
        }
    } else {
        if !needs_guide.is_empty() {
            let names = needs_guide.iter().map(|a| a.display()).collect::<Vec<_>>().join(", ");
            let fixed = fix && guide::install(ctx, &needs_guide, false, bin_path).is_ok();
            c.add("guide", "warn", format!("Falta o está desactualizada la skill guía en: {names}."))
                .hint("skillbank agents install-guide")
                .fix(fixed);
        }
        if !foreign.is_empty() {
            let names = foreign.iter().map(|a| a.display()).collect::<Vec<_>>().join(", ");
            c.add("guide-foreign", "warn", format!("Hay una skill ajena llamada '{}' en: {names}.", guide::GUIDE_NAME))
                .hint("Revísala; para reemplazarla: skillbank agents install-guide --force");
        }
    }

    // Skills y metadatos
    if bank.is_initialized() {
        let names = bank.names().unwrap_or_default();
        let invalid: Vec<String> = names
            .iter()
            .filter(|n| validate_dir(&bank.skill_dir(n), &ValidateOptions::default()).has_errors())
            .cloned()
            .collect();
        if invalid.is_empty() {
            c.add("skills", "ok", "Todas las skills del banco son válidas.");
        } else {
            c.add("skills", "warn", format!("Skills con errores de validación: {}.", invalid.join(", ")))
                .hint("skillbank validate <nombre>");
        }

        let mut missing_meta = Vec::new();
        let mut stale = Vec::new();
        for n in &names {
            let h = tree_hash(&bank.skill_dir(n)).unwrap_or_default();
            match SkillMeta::load(&bank.meta_path(n)) {
                Ok(Some(m)) if m.content_hash == h => {}
                Ok(Some(mut m)) => {
                    stale.push(n.clone());
                    if fix {
                        m.content_hash = h;
                        let _ = bank.save_meta(&m);
                    }
                }
                _ => {
                    missing_meta.push(n.clone());
                    if fix {
                        let mut m = bank.meta(n);
                        m.content_hash = h;
                        let _ = bank.save_meta(&m);
                    }
                }
            }
        }
        if !missing_meta.is_empty() {
            c.add("meta-missing", "warn", format!("Skills sin metadatos: {}.", missing_meta.join(", "))).fix(fix);
        }
        if !stale.is_empty() {
            c.add(
                "meta-stale",
                "warn",
                format!("Metadatos con hash desactualizado (editadas a mano): {}.", stale.join(", ")),
            )
            .fix(fix);
        }
        let orphans: Vec<PathBuf> = fs::read_dir(bank.meta_dir())
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "json"))
            .filter(|p| !bank.has(&p.file_stem().unwrap_or_default().to_string_lossy()))
            .collect();
        if !orphans.is_empty() {
            if fix {
                for p in &orphans {
                    let _ = fs::remove_file(p);
                }
            }
            let list = orphans
                .iter()
                .filter_map(|p| p.file_name())
                .map(|n| n.to_string_lossy().into_owned())
                .collect::<Vec<_>>();
            c.add("meta-orphan", "warn", format!("Metadatos sin skill: {}.", list.join(", "))).fix(fix);
        }

        let leftovers: Vec<PathBuf> =
            fs::read_dir(bank.staging_dir()).into_iter().flatten().flatten().map(|e| e.path()).collect();
        if !leftovers.is_empty() {
            if fix {
                for p in &leftovers {
                    let _ = fs::remove_dir_all(p);
                }
            }
            c.add(
                "staging",
                "warn",
                format!("{} carpeta(s) temporales de operaciones interrumpidas.", leftovers.len()),
            )
            .fix(fix);
        }

        let conflicts: Vec<String> = walkdir::WalkDir::new(&bank.root)
            .into_iter()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().contains(".sbconflict"))
            .map(|e| e.path().strip_prefix(&bank.root).unwrap_or(e.path()).display().to_string())
            .collect();
        if !conflicts.is_empty() {
            c.add("conflicts", "warn", format!("Archivos de conflicto de sincronización: {}.", conflicts.join(", ")))
                .hint("Se podrán resolver con `skillbank sync --conflicts` (Fase 3).");
        }
    }

    // Registro de instalaciones
    let reg_path = ctx.paths.registry_file();
    if let Ok(mut reg) = Registry::load(&reg_path) {
        let missing: Vec<String> = reg
            .installs
            .iter()
            .filter(|r| !Path::new(&r.path).join("SKILL.md").is_file())
            .map(|r| r.path.clone())
            .collect();
        if missing.is_empty() {
            c.add("registry", "ok", format!("Registro de instalaciones: {} entrada(s).", reg.installs.len()));
        } else {
            let mut fixed = false;
            if fix {
                for p in &missing {
                    reg.remove_path(p);
                }
                fixed = reg.save(&reg_path).is_ok();
            }
            c.add("registry", "warn", format!("{} instalación(es) registradas ya no existen en disco.", missing.len()))
                .fix(fixed);
        }
    }

    // rclone (Fase 3)
    match rclone_version() {
        None => {
            c.add("rclone", "info", "rclone no está instalado (hará falta para sincronizar con Google Drive).")
                .hint("sudo -v ; curl https://rclone.org/install.sh | sudo bash");
        }
        Some((v, text)) if v < RCLONE_MIN => {
            c.add("rclone", "warn", format!("{text}: bisync necesita rclone ≥ {}.{}.", RCLONE_MIN.0, RCLONE_MIN.1))
                .hint("Instala la versión oficial de rclone.org (la de apt es antigua).");
        }
        Some((_, text)) => {
            c.add("rclone", "ok", format!("{text}."));
        }
    }
    if let Ok(st) = crate::sync::SyncState::load(&ctx.paths.sync_state_file())
        && st.dirty
    {
        c.add("sync", "info", "Hay cambios en el banco pendientes de sincronizar.");
    }

    let checks = c.0;
    DoctorReport {
        errors: checks.iter().filter(|x| x.status == "error").count(),
        warnings: checks.iter().filter(|x| x.status == "warn").count(),
        fixed: checks.iter().filter(|x| x.fixed).count(),
        checks,
    }
}
