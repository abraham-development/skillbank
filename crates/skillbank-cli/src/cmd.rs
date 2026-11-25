//! Implementación de cada comando sobre `skillbank-core`.

use crate::cli::*;
use crate::output::{Reply, render_diagnostics};
use serde_json::{Value, json};
use skillbank_core::agents::{
    AgentId, AgentRequest, Scope, agent_from_env, detect_project_agents, plan_destinations, primary_dir, resolve_agents,
};
use skillbank_core::error::{CoreError, ErrorKind, Result};
use skillbank_core::install::{PullAction, PullOptions, PushOptions, PushStatus};
use skillbank_core::paths::{pretty, project_root};
use skillbank_core::skill::resolve_skill_dir;
use skillbank_core::skill::validate::{Profile, Severity, ValidateOptions, validate_dir};
use skillbank_core::{Context, guide, hash, install, search};
use std::path::{Path, PathBuf};

fn cwd() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn p(ctx: &Context, path: &Path) -> String {
    pretty(path, &ctx.paths.home)
}

fn ps(ctx: &Context, path: &str) -> String {
    pretty(Path::new(path), &ctx.paths.home)
}

fn parse_agents(values: &[String]) -> Result<AgentRequest> {
    AgentRequest::parse(values).map_err(CoreError::usage)
}

fn diag_warnings(diags: &[skillbank_core::skill::validate::Diagnostic]) -> Vec<Value> {
    diags
        .iter()
        .filter(|d| d.severity == Severity::Warning)
        .map(|d| serde_json::to_value(d).unwrap_or(Value::Null))
        .collect()
}

fn agents_label(a: &[AgentId]) -> String {
    a.iter().map(|x| x.display()).collect::<Vec<_>>().join(", ")
}

fn trunc(s: &str, n: usize) -> String {
    let one_line = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.chars().count() <= n {
        one_line
    } else {
        format!("{}…", one_line.chars().take(n - 1).collect::<String>())
    }
}

// ───────────── push ─────────────

pub fn push(ctx: &Context, a: &PushArgs) -> Result<Reply> {
    let agent = match &a.agent {
        Some(s) => Some(s.parse::<AgentId>().map_err(CoreError::usage)?),
        None => agent_from_env(),
    };
    let opts = PushOptions {
        force: a.force,
        tags: a.tags.as_ref().map(|t| vec![t.clone()]),
        add_tags: a.add_tags,
        allow_invalid: a.allow_invalid,
        dry_run: a.dry_run,
        full_diff: a.diff,
        cwd: cwd(),
        agent,
    };
    let out = install::push(ctx, &a.target, &opts)?;
    let verb = match (out.status, out.dry_run) {
        (PushStatus::Created, false) => "subida al banco (nueva)",
        (PushStatus::Updated, false) => "actualizada en el banco",
        (PushStatus::Unchanged, _) => "sin cambios: el banco ya tenía esta versión",
        (PushStatus::Created, true) => "se subiría al banco (nueva) [simulación]",
        (PushStatus::Updated, true) => "se actualizaría en el banco [simulación]",
    };
    let mut human = format!("  Origen: {}\n  Banco:  {}\n", ps(ctx, &out.source), ps(ctx, &out.bank_path));
    if !out.tags.is_empty() {
        human.push_str(&format!("  Etiquetas: {}\n", out.tags.join(", ")));
    }
    if let Some(w) = &out.registry_warning {
        human.push_str(&format!("  ⚠ No se pudo actualizar el registro de instalaciones ({w}).\n"));
    }
    let mut warnings = diag_warnings(&out.warnings);
    if let Some(w) = &out.registry_warning {
        warnings.push(json!({"code": "REGISTRY_NOT_WRITTEN", "message": w}));
    }
    Ok(Reply::new(format!("✓ Skill '{}' {verb}.", out.name), &out).human(human).warnings(warnings))
}

// ───────────── pull ─────────────

pub fn pull(ctx: &Context, a: &PullArgs) -> Result<Reply> {
    let opts = PullOptions {
        agents: parse_agents(&a.agent)?,
        scope: if a.global { Scope::Global } else { Scope::Project },
        project: a.project.clone(),
        cwd: cwd(),
        force: a.force,
        dry_run: a.dry_run,
        dedupe: !a.no_dedupe,
    };
    let out = install::pull(ctx, &a.names, &opts)?;
    let mut human = String::new();
    for it in &out.items {
        let act = match it.action {
            PullAction::Installed => "instalada",
            PullAction::Updated if it.reason.as_deref() == Some("forced") => "sobrescrita",
            PullAction::Updated => "actualizada",
            PullAction::Unchanged => "ya estaba al día",
            PullAction::Conflict => "conflicto",
        };
        human.push_str(&format!("  • {} → {}  [{act}] ({})\n", it.skill, ps(ctx, &it.path), agents_label(&it.agents)));
    }
    if let Some(w) = &out.registry_warning {
        human.push_str(&format!("  ⚠ No se pudo actualizar el registro de instalaciones ({w}).\n"));
    }
    let mut warnings = diag_warnings(&out.warnings);
    if let Some(w) = &out.registry_warning {
        warnings.push(json!({"code": "REGISTRY_NOT_WRITTEN", "message": w}));
    }
    let n = a.names.len();
    let msg = if out.dry_run {
        format!("Simulación: así se instalaría{} (no se escribió nada).", if n > 1 { "n" } else { "" })
    } else if n == 1 {
        format!("✓ Skill '{}' traída del banco.", a.names[0])
    } else {
        format!("✓ {n} skills traídas del banco.")
    };
    Ok(Reply::new(msg, &out).human(human).warnings(warnings))
}

// ───────────── list / search / show / path ─────────────

pub fn list(ctx: &Context, a: &ListArgs) -> Result<Reply> {
    let mut all = ctx.bank.list()?;
    if let Some(t) = &a.tag {
        let t = t.to_lowercase();
        all.retain(|s| s.tags.contains(&t));
    }
    if a.invalid {
        all.retain(|s| !s.valid);
    }
    if all.is_empty() {
        let msg = if ctx.bank.names()?.is_empty() {
            "El banco está vacío. Sube tu primera skill con `skillbank push <carpeta>`."
        } else {
            "Ninguna skill coincide con el filtro."
        };
        return Ok(Reply::new(msg, &all));
    }
    let width = all.iter().map(|s| s.name.chars().count()).max().unwrap_or(10).min(40);
    let mut human = String::new();
    for s in &all {
        let mut badges = Vec::new();
        if !s.valid {
            badges.push("inválida".to_string());
        }
        if !s.claude_code_fields.is_empty() {
            badges.push("solo Claude Code".to_string());
        }
        let tags = if s.tags.is_empty() { String::new() } else { format!(" #{}", s.tags.join(" #")) };
        let b = if badges.is_empty() { String::new() } else { format!(" [{}]", badges.join(", ")) };
        human.push_str(&format!(
            "  {:<width$}  {}{tags}{b}\n",
            s.name,
            trunc(s.description.as_deref().unwrap_or("(sin descripción)"), 80),
        ));
    }
    Ok(Reply::new(format!("{} skill(s) en el banco ({}):", all.len(), p(ctx, &ctx.bank.root)), &all).human(human))
}

pub fn search_cmd(ctx: &Context, a: &SearchArgs) -> Result<Reply> {
    let hits = search::search(&ctx.bank, &a.query, a.tag.as_deref(), a.limit)?;
    if hits.is_empty() {
        return Ok(Reply::new(format!("Sin resultados para \"{}\".", a.query), &hits));
    }
    let mut human = String::new();
    for h in &hits {
        human.push_str(&format!("  {}  — {}\n", h.name, trunc(h.description.as_deref().unwrap_or(""), 90)));
        if let Some(sn) = &h.snippet {
            human.push_str(&format!("      «{}»\n", trunc(sn, 100)));
        }
    }
    Ok(Reply::new(format!("{} resultado(s) para \"{}\":", hits.len(), a.query), &hits).human(human))
}

pub fn show(ctx: &Context, a: &ShowArgs) -> Result<Reply> {
    let d = ctx.bank.detail(&a.name, a.body)?;
    let reg = skillbank_core::registry::Registry::load(&ctx.paths.registry_file()).unwrap_or_default();
    let installs: Vec<Value> = reg
        .for_skill(&a.name)
        .into_iter()
        .map(|r| {
            let disk = hash::tree_hash(Path::new(&r.path)).ok();
            let status = skillbank_core::registry::InstallStatus::compute(
                disk.as_deref(),
                &r.installed_hash,
                Some(&d.summary.content_hash),
            );
            json!({ "path": r.path, "scope": r.scope, "agents": r.agents, "status": status, "status_es": status.label_es() })
        })
        .collect();
    let mut human = String::new();
    human.push_str(&format!("  {}\n", d.summary.description.as_deref().unwrap_or("(sin descripción)")));
    human.push_str(&format!("  Ruta: {}\n", ps(ctx, &d.summary.path)));
    if !d.summary.tags.is_empty() {
        human.push_str(&format!("  Etiquetas: {}\n", d.summary.tags.join(", ")));
    }
    human.push_str(&format!(
        "  Actualizada: {}   Hash: {}\n",
        d.summary.updated_at.format("%Y-%m-%d %H:%M"),
        hash::short(&d.summary.content_hash)
    ));
    if !d.summary.claude_code_fields.is_empty() {
        human.push_str(&format!("  Campos solo Claude Code: {}\n", d.summary.claude_code_fields.join(", ")));
    }
    human.push_str("  Archivos:\n");
    for f in &d.files {
        human.push_str(&format!("    {}{}  ({} B)\n", f.path, if f.executable { " *" } else { "" }, f.size));
    }
    if !installs.is_empty() {
        human.push_str("  Instalada en:\n");
        for i in &installs {
            human.push_str(&format!(
                "    {}  [{}]\n",
                ps(ctx, i["path"].as_str().unwrap_or("")),
                i["status_es"].as_str().unwrap_or("")
            ));
        }
    }
    let diags: Vec<Value> = d.diagnostics.iter().map(|x| serde_json::to_value(x).unwrap()).collect();
    if !diags.is_empty() {
        human.push_str("  Validación:\n");
        human.push_str(&render_diagnostics(&diags));
    }
    if let Some(b) = &d.body {
        human.push('\n');
        human.push_str(b);
    }
    let mut data = serde_json::to_value(&d)?;
    data["installs"] = Value::Array(installs);
    Ok(Reply::new(format!("Skill '{}'", a.name), data).human(human))
}

pub fn path(ctx: &Context, name: &str) -> Result<Reply> {
    let dir = ctx.bank.require(name)?;
    Ok(Reply::new(dir.display().to_string(), json!({ "name": name, "path": dir })))
}

// ───────────── validate ─────────────

pub fn validate(ctx: &Context, a: &ValidateArgs) -> Result<Reply> {
    let profile: Profile = a.profile.parse().map_err(CoreError::usage)?;
    let targets = match parse_agents(&a.agent)? {
        AgentRequest::List(l) => l,
        AgentRequest::All => AgentId::ALL.to_vec(),
        _ => vec![],
    };
    let as_path = cwd().join(&a.target);
    let dir = if let Some(d) = resolve_skill_dir(&as_path) {
        d
    } else if !a.target.contains('/') && ctx.bank.has(&a.target) {
        ctx.bank.skill_dir(&a.target)
    } else if as_path.exists() {
        as_path
    } else {
        return Err(CoreError::not_found(format!(
            "No encontré la skill '{}' (ni como carpeta ni en el banco).",
            a.target
        )));
    };
    let report = validate_dir(&dir, &ValidateOptions { profile, target_agents: targets, ..Default::default() });
    let errors = report.count(Severity::Error);
    let warnings = report.count(Severity::Warning);
    let fails = errors > 0 || (a.strict && warnings > 0);
    let data = json!({ "path": dir, "profile": profile, "valid": errors == 0, "report": report });
    if fails {
        let msg = if errors > 0 {
            format!("La skill tiene {errors} error(es) y {warnings} aviso(s).")
        } else {
            format!("La skill tiene {warnings} aviso(s) (modo --strict).")
        };
        let mut e = CoreError::new(ErrorKind::Invalid, msg).with_data(json!({
            "path": dir, "profile": profile, "valid": errors == 0, "report": report, "diagnostics": report.diagnostics
        }));
        e.hint = Some("Corrige los puntos marcados y vuelve a validar.".into());
        return Err(e);
    }
    let diags: Vec<Value> = report.diagnostics.iter().map(|x| serde_json::to_value(x).unwrap()).collect();
    let msg = if warnings == 0 {
        format!("✓ Skill válida ({}).", report.name.as_deref().unwrap_or("?"))
    } else {
        format!("✓ Skill válida ({}) con {warnings} aviso(s).", report.name.as_deref().unwrap_or("?"))
    };
    Ok(Reply::new(msg, data).human(render_diagnostics(&diags)))
}

// ───────────── remove ─────────────

pub fn remove(ctx: &Context, a: &RemoveArgs) -> Result<Reply> {
    if !a.yes {
        ctx.bank.require(&a.name)?;
        return Err(CoreError::usage(format!("Para eliminar '{}' del banco, confirma con --yes.", a.name))
            .with_hint("Se moverá a la papelera local durante 7 días."));
    }
    let out = install::remove(ctx, &a.name)?;
    let mut human = format!("  Papelera: {}\n", ps(ctx, &out.trash_path));
    if out.installs_left > 0 {
        human.push_str(&format!("  Sigue instalada en {} lugar(es); esas copias no se tocan.\n", out.installs_left));
    }
    Ok(Reply::new(format!("✓ Skill '{}' eliminada del banco.", a.name), &out).human(human))
}

// ───────────── agents ─────────────

pub fn agents(ctx: &Context, cmd: &AgentsCmd) -> Result<Reply> {
    match cmd {
        AgentsCmd::List => {
            let mut rows = Vec::new();
            let mut human = String::new();
            let root = project_root(&cwd());
            for a in AgentId::ALL {
                let ap = ctx.config.agent(a);
                let present = a.is_present(&ctx.paths.home);
                let g = primary_dir(&ap, Scope::Global, &root, &ctx.paths);
                let guide_state = guide::state_for_agent(ctx, a);
                human.push_str(&format!(
                    "  {:<12} {}  proyecto: {:<16} global: {}  guía: {}\n",
                    a.display(),
                    if present { "✓ instalado   " } else { "· no detectado" },
                    ap.project_dir,
                    p(ctx, &g),
                    guide_state
                ));
                rows.push(json!({
                    "agent": a, "name": a.display(), "present": present,
                    "project_dir": ap.project_dir, "global_dir": g,
                    "also_reads_project": ap.also_reads_project, "also_reads_global": ap.also_reads_global,
                    "guide": guide_state,
                }));
            }
            Ok(Reply::new("Agentes:", rows).human(human))
        }
        AgentsCmd::Detect { dir } => {
            let root = project_root(dir.as_deref().unwrap_or(&cwd()));
            let detected = detect_project_agents(&root);
            let (agents, source) = resolve_agents(&AgentRequest::Auto, Some(&root), &ctx.config);
            let plan = plan_destinations(&agents, Scope::Project, &root, &ctx.config, &ctx.paths, true);
            let mut human = format!(
                "  Proyecto: {}\n  Detectados: {}\n  Se usarían: {} ({:?})\n",
                p(ctx, &root),
                if detected.is_empty() { "ninguno".into() } else { agents_label(&detected) },
                agents_label(&agents),
                source
            );
            for d in &plan {
                human.push_str(&format!("    → {} ({})\n", p(ctx, &d.dir), agents_label(&d.agents)));
            }
            Ok(Reply::new(
                "Detección de agentes:",
                json!({"project_root": root, "detected": detected, "agents": agents, "agent_source": source, "destinations": plan}),
            )
            .human(human))
        }
        AgentsCmd::InstallGuide { agent, all, force, uninstall } => {
            let agents: Vec<AgentId> = if *all {
                AgentId::ALL.to_vec()
            } else if !agent.is_empty() {
                match parse_agents(agent)? {
                    AgentRequest::List(l) => l,
                    AgentRequest::All => AgentId::ALL.to_vec(),
                    _ => guide::present_agents(ctx),
                }
            } else {
                let p = guide::present_agents(ctx);
                if p.is_empty() { vec![AgentId::Claude] } else { p }
            };
            let bin = std::env::current_exe()
                .ok()
                .and_then(|p| std::fs::canonicalize(p).ok())
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "skillbank".into());
            let results =
                if *uninstall { guide::uninstall(ctx, &agents)? } else { guide::install(ctx, &agents, *force, &bin)? };
            let mut human = String::new();
            for r in &results {
                let act = match r.action.as_str() {
                    "installed" => "instalada",
                    "updated" => "actualizada",
                    "unchanged" => "ya estaba al día",
                    "removed" => "quitada",
                    _ => "no estaba",
                };
                human.push_str(&format!("  • {}  [{act}] ({})\n", ps(ctx, &r.dir), agents_label(&r.agents)));
            }
            let msg = if *uninstall {
                "Skill guía quitada.".to_string()
            } else {
                format!("✓ Skill guía '{}' lista para: {}.", guide::GUIDE_NAME, agents_label(&agents))
            };
            Ok(Reply::new(msg, &results).human(human))
        }
    }
}

// ───────────── config ─────────────

pub fn config(ctx: &Context, cmd: &ConfigCmd) -> Result<Reply> {
    match cmd {
        ConfigCmd::Path => {
            let data = json!({
                "config_file": ctx.paths.config_file,
                "config_exists": ctx.paths.config_file.exists(),
                "bank": ctx.bank.root,
                "bank_initialized": ctx.bank.is_initialized(),
                "state_dir": ctx.paths.state_dir,
                "registry": ctx.paths.registry_file(),
            });
            let human = format!(
                "  Configuración: {}{}\n  Banco:         {}{}\n  Estado:        {}\n",
                p(ctx, &ctx.paths.config_file),
                if ctx.paths.config_file.exists() { "" } else { "  (no existe; se usan valores por defecto)" },
                p(ctx, &ctx.bank.root),
                if ctx.bank.is_initialized() { "" } else { "  (se creará al subir la primera skill)" },
                p(ctx, &ctx.paths.state_dir),
            );
            Ok(Reply::new("Rutas de Banco de Skills:", data).human(human))
        }
        ConfigCmd::Show => {
            let text = toml::to_string_pretty(&ctx.config).unwrap_or_default();
            Ok(Reply::new("Configuración efectiva:", &ctx.config).human(text))
        }
    }
}
