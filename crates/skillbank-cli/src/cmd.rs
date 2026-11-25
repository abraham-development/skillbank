//! Implementación de cada comando sobre `skillbank-core`.

use crate::cli::*;
use crate::output::{Reply, render_checks, render_diagnostics, render_import_rows};
use serde_json::{Value, json};
use skillbank_core::agents::{
    AgentId, AgentRequest, Scope, agent_from_env, detect_project_agents, plan_destinations, primary_dir, resolve_agents,
};
use skillbank_core::error::{CoreError, ErrorKind, Result};
use skillbank_core::install::{PullAction, PullOptions, PushOptions, PushStatus};
use skillbank_core::paths::{pretty, project_root};
use skillbank_core::skill::resolve_skill_dir;
use skillbank_core::skill::validate::{Profile, Severity, ValidateOptions, validate_dir};
use skillbank_core::{Context, archive, doctor as core_doctor, guide, hash, install, manage, search, status};
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

pub fn push(ctx: &Context, a: &PushArgs, agents: &[String]) -> Result<Reply> {
    let agent = identity(agents)?;
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

pub fn pull(ctx: &Context, a: &PullArgs, agents: &[String]) -> Result<Reply> {
    let opts = PullOptions {
        agents: parse_agents(agents)?,
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

pub fn validate(ctx: &Context, a: &ValidateArgs, agents: &[String]) -> Result<Reply> {
    let profile: Profile = a.profile.parse().map_err(CoreError::usage)?;
    let targets = match parse_agents(agents)? {
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

pub fn agents(ctx: &Context, cmd: &AgentsCmd, agent_args: &[String]) -> Result<Reply> {
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
        AgentsCmd::InstallGuide { all, force, uninstall } => {
            let agents: Vec<AgentId> = if *all {
                AgentId::ALL.to_vec()
            } else if !agent_args.is_empty() {
                match parse_agents(agent_args)? {
                    AgentRequest::List(l) => l,
                    AgentRequest::All => AgentId::ALL.to_vec(),
                    _ => guide::present_agents(ctx),
                }
            } else {
                let p = guide::present_agents(ctx);
                if p.is_empty() { vec![AgentId::Claude] } else { p }
            };
            let bin = self_bin();
            let results =
                if *uninstall { guide::uninstall(ctx, &agents)? } else { guide::install(ctx, &agents, *force, &bin)? };
            let mut human = String::new();
            for r in &results {
                let act = match r.action.as_str() {
                    "installed" => "instalada",
                    "updated" => "actualizada",
                    "unchanged" => "ya estaba al día",
                    "removed" => "quitada",
                    "removed_legacy" => "copia antigua quitada",
                    _ => "no estaba",
                };
                let who = if r.agents.is_empty() { String::new() } else { format!(" ({})", agents_label(&r.agents)) };
                human.push_str(&format!("  • {}  [{act}]{who}\n", ps(ctx, &r.dir)));
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

fn self_bin() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|p| std::fs::canonicalize(p).ok())
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "skillbank".into())
}

fn split_list(values: &[String]) -> Vec<String> {
    values.iter().flat_map(|v| v.split(',')).map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect()
}

// ───────────── tag / rename / new ─────────────

pub fn tag(ctx: &Context, a: &TagArgs) -> Result<Reply> {
    let Some(name) = &a.name else {
        let tags = manage::all_tags(ctx)?;
        let human: String = tags.iter().map(|(t, n)| format!("  #{t}  ({n})\n")).collect();
        let data: Vec<Value> = tags.iter().map(|(t, n)| json!({"tag": t, "count": n})).collect();
        let msg = if tags.is_empty() {
            "No hay etiquetas todavía.".to_string()
        } else {
            format!("{} etiqueta(s):", tags.len())
        };
        return Ok(Reply::new(msg, data).human(human));
    };
    let opts = manage::TagOptions {
        add: split_list(&a.add),
        remove: split_list(&a.remove),
        set: a.set.as_ref().map(|s| split_list(std::slice::from_ref(s))),
    };
    let out = manage::tag(ctx, name, &opts)?;
    let shown = if out.tags.is_empty() { "(sin etiquetas)".to_string() } else { format!("#{}", out.tags.join(" #")) };
    let msg = if out.changed {
        format!("✓ Etiquetas de '{name}': {shown}")
    } else {
        format!("Etiquetas de '{name}': {shown}")
    };
    Ok(Reply::new(msg, &out))
}

pub fn rename(ctx: &Context, old: &str, new: &str) -> Result<Reply> {
    let out = manage::rename(ctx, old, new)?;
    let mut human = format!("  Ruta: {}\n", ps(ctx, &out.path));
    if out.installs_with_old_name > 0 {
        human.push_str(&format!(
            "  {} copia(s) instaladas siguen llamándose '{old}'; tráelas de nuevo con `skillbank pull {new}`.\n",
            out.installs_with_old_name
        ));
    }
    Ok(Reply::new(format!("✓ '{old}' ahora se llama '{new}'."), &out).human(human))
}

pub fn new(ctx: &Context, a: &NewArgs) -> Result<Reply> {
    let dest =
        if a.in_bank { manage::NewDest::Bank } else { manage::NewDest::Dir(abs_opt(&a.dir).unwrap_or_else(cwd)) };
    let opts = manage::NewOptions {
        template: a.template.clone(),
        description: a.description.clone(),
        tags: a.tags.as_ref().map(|t| split_list(std::slice::from_ref(t))).unwrap_or_default(),
        dest,
    };
    let out = manage::new_skill(ctx, &a.name, &opts)?;
    let mut human = format!("  Ruta: {}\n  Archivos: {}\n", ps(ctx, &out.path), out.files.join(", "));
    human.push_str(if out.in_bank {
        "  Edita SKILL.md y tráela a un proyecto con `skillbank pull`.\n"
    } else {
        "  Edita SKILL.md y, cuando esté lista, súbela con `skillbank push`.\n"
    });
    Ok(Reply::new(format!("✓ Skill '{}' creada con la plantilla '{}'.", out.name, out.template), &out).human(human))
}

// ───────────── export / import ─────────────

fn is_zip_path(p: &Path) -> bool {
    p.extension().is_some_and(|e| e.eq_ignore_ascii_case("zip"))
}

pub fn export(ctx: &Context, a: &ExportArgs) -> Result<Reply> {
    if a.all {
        let out = match &a.output {
            Some(p) if is_zip_path(p) => p.clone(),
            other => {
                let dir = other.clone().unwrap_or_else(cwd);
                dir.join(format!("banco-de-skills-respaldo-{}.zip", chrono_stamp()))
            }
        };
        let r = archive::export_backup(ctx, &out)?;
        return Ok(Reply::new(format!("✓ Respaldo de {} skill(s) en {}.", r.skills, ps(ctx, &r.path)), &r));
    }
    if a.names.len() > 1 && a.output.as_deref().is_some_and(is_zip_path) {
        return Err(CoreError::usage("Con varias skills, -o debe ser una carpeta (se crea un .zip por skill)."));
    }
    let opts = archive::ExportOptions { claude_ai: a.claude_ai, strip_nonstandard: a.strip_nonstandard };
    let mut results = Vec::new();
    for name in &a.names {
        let out = match &a.output {
            Some(p) if is_zip_path(p) => p.clone(),
            other => other.clone().unwrap_or_else(cwd).join(format!("{name}.zip")),
        };
        results.push(archive::export_skill(ctx, name, &out, &opts)?);
    }
    let mut human = String::new();
    let mut warnings = Vec::new();
    for r in &results {
        human.push_str(&format!("  • {} → {} ({} archivos)\n", r.name, ps(ctx, &r.path), r.files));
        if !r.stripped_keys.is_empty() {
            human.push_str(&format!("    campos quitados en la copia: {}\n", r.stripped_keys.join(", ")));
        }
        warnings.extend(diag_warnings(&r.warnings));
    }
    let msg = if a.claude_ai { "✓ Exportado para claude.ai." } else { "✓ Exportado." };
    Ok(Reply::new(msg, &results).human(human).warnings(warnings))
}

fn chrono_stamp() -> String {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    // AAAAMMDD-HHMM en UTC sin depender de chrono en el CLI.
    let days = now / 86_400;
    let secs = now % 86_400;
    let (y, m, d) = civil_from_days(days as i64);
    format!("{y:04}{m:02}{d:02}-{:02}{:02}", secs / 3600, (secs % 3600) / 60)
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

pub fn import(ctx: &Context, a: &ImportArgs) -> Result<Reply> {
    let opts = archive::ImportOptions {
        force: a.force,
        skip_existing: a.skip_existing,
        only: a.only.as_ref().map(|o| split_list(std::slice::from_ref(o))),
        tags: a.tags.as_ref().map(|t| split_list(std::slice::from_ref(t))),
        dry_run: a.dry_run,
    };
    let path = if a.source.is_absolute() { a.source.clone() } else { cwd().join(&a.source) };
    let out = archive::import(ctx, &path, &opts)?;
    let rows: Vec<Value> = out.rows.iter().map(|r| serde_json::to_value(r).unwrap()).collect();
    let msg = if out.dry_run {
        format!("Simulación: se importarían {} y se sobrescribirían {}.", out.imported, out.updated)
    } else {
        format!("✓ Importadas {} skill(s) nuevas, {} sobrescritas.", out.imported, out.updated)
    };
    Ok(Reply::new(msg, &out).human(render_import_rows(&rows)))
}

// ───────────── installed / outdated / scan ─────────────

fn render_installs(ctx: &Context, items: &[status::InstallView]) -> String {
    items
        .iter()
        .map(|v| {
            format!(
                "  • {} → {}  [{}] ({})\n",
                v.record.skill,
                ps(ctx, &v.record.path),
                v.status_es,
                agents_label(&v.record.agents)
            )
        })
        .collect()
}

fn abs_opt(p: &Option<PathBuf>) -> Option<PathBuf> {
    p.as_ref().map(|p| if p.is_absolute() { p.clone() } else { cwd().join(p) })
}

pub fn installed(ctx: &Context, a: &InstalledArgs) -> Result<Reply> {
    let items = status::installed(ctx, a.name.as_deref(), abs_opt(&a.project).as_deref())?;
    if items.is_empty() {
        return Ok(Reply::new("No hay instalaciones registradas.", &items));
    }
    Ok(Reply::new(format!("{} instalación(es):", items.len()), &items).human(render_installs(ctx, &items)))
}

pub fn outdated(ctx: &Context, a: &OutdatedArgs) -> Result<Reply> {
    let out = status::outdated(ctx, abs_opt(&a.project).as_deref(), a.update)?;
    let pending: Vec<status::InstallView> =
        out.items.iter().filter(|v| v.status != skillbank_core::registry::InstallStatus::UpToDate).cloned().collect();
    let mut human = render_installs(ctx, &out.items);
    if !a.update && pending.iter().any(|v| v.status == skillbank_core::registry::InstallStatus::Outdated) {
        human.push_str("  → Las «desactualizadas» se actualizan sin riesgo con --update.\n");
    }
    if pending.iter().any(|v| {
        matches!(
            v.status,
            skillbank_core::registry::InstallStatus::Modified | skillbank_core::registry::InstallStatus::Diverged
        )
    }) {
        human.push_str("  → Las «modificadas»/«divergentes» tienen cambios locales: súbelas con push o tráelas con pull --force.\n");
    }
    let msg = if out.items.is_empty() {
        "✓ Todas las instalaciones están al día.".to_string()
    } else if !out.updated.is_empty() {
        format!("✓ {} actualizada(s); {} pendiente(s).", out.updated.len(), pending.len())
    } else {
        format!("{} instalación(es) no están al día:", out.items.len())
    };
    let mut warnings = vec![];
    if let Some(w) = &out.registry_warning {
        warnings.push(json!({"code": "REGISTRY_NOT_WRITTEN", "message": w}));
    }
    Ok(Reply::new(msg, &out).human(human).warnings(warnings))
}

pub fn scan(ctx: &Context, a: &ScanArgs) -> Result<Reply> {
    let opts = status::ScanOptions {
        global: a.global,
        dir: abs_opt(&a.dir),
        cwd: cwd(),
        register: a.register,
        import: a.import,
    };
    let out = status::scan(ctx, &opts)?;
    let mut human = String::new();
    for e in &out.entries {
        let st = match e.status.as_str() {
            "same" => "= igual al banco",
            "different" => "≠ distinta del banco",
            "absent" => "+ no está en el banco",
            _ => "✗ inválida",
        };
        let mut extra = Vec::new();
        if e.synced {
            extra.push("sincronizada desde claude.ai".to_string());
        }
        if let Some(r) = &e.real_path {
            extra.push(format!("enlace a {}", ps(ctx, r)));
        }
        if let Some(act) = &e.action {
            extra.push(if act == "imported" { "importada".into() } else { "registrada".into() });
        }
        let extra = if extra.is_empty() { String::new() } else { format!("  ({})", extra.join("; ")) };
        human.push_str(&format!("  {:<28} {st}  {}{extra}\n", e.name, ps(ctx, &e.path)));
    }
    let absent = out.entries.iter().filter(|e| e.status == "absent").count();
    if absent > 0 && !a.import {
        human.push_str(&format!("  → {absent} skill(s) no están en el banco: súbelas con --import.\n"));
    }
    let msg = if out.entries.is_empty() {
        "No encontré skills en las carpetas de los agentes.".to_string()
    } else {
        format!(
            "{} skill(s) encontradas{}{}:",
            out.entries.len(),
            if out.imported > 0 { format!(", {} importadas", out.imported) } else { String::new() },
            if out.registered > 0 { format!(", {} registradas", out.registered) } else { String::new() }
        )
    };
    Ok(Reply::new(msg, &out).human(human))
}

// ───────────── doctor ─────────────

pub fn doctor(ctx: &Context, fix: bool) -> Result<Reply> {
    let r = core_doctor::doctor(ctx, fix, &self_bin());
    let checks: Vec<Value> = r.checks.iter().map(|c| serde_json::to_value(c).unwrap()).collect();
    if r.errors > 0 {
        return Err(CoreError::new(
            ErrorKind::Error,
            format!("Diagnóstico: {} error(es), {} aviso(s).", r.errors, r.warnings),
        )
        .with_data(serde_json::to_value(&r)?));
    }
    let msg = match (r.warnings, r.fixed) {
        (0, 0) => "✓ Todo en orden.".to_string(),
        (0, f) => format!("✓ Todo en orden ({f} arreglo(s) aplicados)."),
        (w, f) => {
            format!("Diagnóstico: {w} aviso(s){}.", if f > 0 { format!(", {f} arreglados") } else { String::new() })
        }
    };
    Ok(Reply::new(msg, &r).human(render_checks(&checks)))
}

// ───────────── identidad y connect ─────────────

/// Agente que está usando el CLI: el único `--agent` dado, o el detectado por el entorno.
pub fn identity(agents: &[String]) -> Result<Option<AgentId>> {
    match parse_agents(agents)? {
        AgentRequest::List(l) if l.len() == 1 => Ok(Some(l[0])),
        _ => Ok(agent_from_env()),
    }
}

pub fn connect(ctx: &Context, agents: &[String]) -> Result<Reply> {
    let Some(agent) = identity(agents)? else {
        return Err(CoreError::usage("No sé qué agente eres. Repite con --agent claude, cursor o antigravity."));
    };
    let cwd = cwd();
    let first_time = skillbank_core::activity::record(
        &ctx.paths,
        agent,
        &skillbank_core::activity::Usage {
            command: "connect".into(),
            project: Some(project_root(&cwd).display().to_string()),
            ok: true,
            connect: true,
            ..Default::default()
        },
    )
    .map_err(|e| e.context("no se pudo guardar la conexión"))?;
    let st = skillbank_core::activity::status(ctx, agent);
    let mut human = format!("  Banco: {}\n", p(ctx, &ctx.bank.root));
    let mut warnings = vec![];
    if st.guide != "current" {
        let msg = format!(
            "La skill guía no está al día en {} (estado: {}). Instálala con `skillbank agents install-guide --agent {}`.",
            agent.display(),
            st.guide,
            agent.key()
        );
        human.push_str(&format!("  ⚠ {msg}\n"));
        warnings.push(json!({"code": "GUIDE_NOT_CURRENT", "message": msg}));
    }
    let msg = if first_time {
        format!("✓ {} conectado con Banco de Skills por primera vez.", agent.display())
    } else {
        format!("✓ {} conectado con Banco de Skills.", agent.display())
    };
    Ok(Reply::new(
        msg,
        json!({
            "agent": agent, "name": agent.display(), "connected_at": st.activity.connected_at,
            "first_time": first_time, "guide": st.guide, "bank": ctx.bank.root,
        }),
    )
    .human(human)
    .warnings(warnings))
}
