//! Gestión de skills del banco: etiquetas, renombrar y crear desde plantillas.

use crate::Context;
use crate::error::{CoreError, ErrorKind, IoContext, Result};
use crate::hash::tree_hash;
use crate::meta::{SkillMeta, Source, hostname, normalize_tags};
use crate::registry::Registry;
use crate::skill::frontmatter::set_name;
use crate::skill::validate::{Diagnostic, Severity, ValidateOptions, validate_dir, validate_text};
use chrono::Utc;
use serde::Serialize;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};

// ───────────────────────────── etiquetas ─────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct TagOptions {
    pub add: Vec<String>,
    pub remove: Vec<String>,
    pub set: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TagOutcome {
    pub name: String,
    pub tags: Vec<String>,
    pub changed: bool,
}

pub fn tag(ctx: &Context, name: &str, opts: &TagOptions) -> Result<TagOutcome> {
    ctx.bank.require(name)?;
    let _lock = ctx.lock_exclusive()?;
    let mut meta = ctx.bank.meta(name);
    let before = meta.tags.clone();
    let mut tags = match &opts.set {
        Some(s) => normalize_tags(s),
        None => meta.tags.clone(),
    };
    tags = normalize_tags(tags.into_iter().chain(opts.add.iter().cloned()));
    let remove = normalize_tags(&opts.remove);
    tags.retain(|t| !remove.contains(t));
    let changed = tags != before;
    if changed || !ctx.bank.meta_path(name).exists() {
        meta.tags = tags.clone();
        if meta.content_hash.is_empty() {
            meta.content_hash = tree_hash(&ctx.bank.skill_dir(name))?;
        }
        ctx.bank.save_meta(&meta)?;
        crate::sync::mark_dirty(&ctx.paths.sync_state_file());
    }
    Ok(TagOutcome { name: name.to_string(), tags, changed })
}

/// Todas las etiquetas del banco con su número de skills.
pub fn all_tags(ctx: &Context) -> Result<Vec<(String, usize)>> {
    let mut counts: std::collections::BTreeMap<String, usize> = Default::default();
    for n in ctx.bank.names()? {
        for t in ctx.bank.meta(&n).tags {
            *counts.entry(t).or_default() += 1;
        }
    }
    Ok(counts.into_iter().collect())
}

// ───────────────────────────── nombres ─────────────────────────────

/// Errores de un nombre de skill (vacío si es válido), con las mismas reglas del validador.
pub fn name_errors(name: &str) -> Vec<Diagnostic> {
    let probe = format!("---\nname: \"{}\"\ndescription: prueba cuando\n---\ncuerpo\n", name.replace('"', "\\\""));
    validate_text(&probe, &ValidateOptions { dir_name: Some(name.to_string()), ..Default::default() })
        .diagnostics
        .into_iter()
        .filter(|d| d.severity == Severity::Error && d.code.starts_with("E_NAME"))
        .collect()
}

fn check_new_name(ctx: &Context, name: &str) -> Result<()> {
    let errs = name_errors(name);
    if !errs.is_empty() {
        return Err(CoreError::new(ErrorKind::Invalid, format!("'{name}' no es un nombre de skill válido."))
            .with_hint(errs.iter().filter_map(|d| d.hint.clone()).next().unwrap_or_else(|| errs[0].message.clone()))
            .with_data(json!({ "diagnostics": errs })));
    }
    if ctx.bank.has(name) {
        return Err(CoreError::new(ErrorKind::Conflict, format!("Ya existe una skill '{name}' en el banco.")));
    }
    Ok(())
}

// ───────────────────────────── renombrar ─────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct RenameOutcome {
    pub old: String,
    pub new: String,
    pub path: String,
    /// Copias instaladas que siguen con el nombre anterior (no se tocan).
    pub installs_with_old_name: usize,
}

pub fn rename(ctx: &Context, old: &str, new: &str) -> Result<RenameOutcome> {
    let old_dir = ctx.bank.require(old)?;
    check_new_name(ctx, new)?;
    let _lock = ctx.lock_exclusive()?;
    let md_path = old_dir.join("SKILL.md");
    let text = fs::read_to_string(&md_path).at(&md_path)?;
    let updated = set_name(&text, new)
        .ok_or_else(|| CoreError::new(ErrorKind::Invalid, "SKILL.md no tiene un campo `name` que se pueda cambiar."))?;
    crate::fsutil::write_atomic(&md_path, updated.as_bytes())?;
    let new_dir = ctx.bank.skill_dir(new);
    fs::rename(&old_dir, &new_dir).at(&new_dir)?;
    let mut meta = ctx.bank.meta(old);
    meta.name = new.to_string();
    meta.updated_at = Utc::now();
    meta.content_hash = tree_hash(&new_dir)?;
    ctx.bank.save_meta(&meta)?;
    let _ = fs::remove_file(ctx.bank.meta_path(old));
    crate::sync::mark_dirty(&ctx.paths.sync_state_file());
    let reg = Registry::load(&ctx.paths.registry_file()).unwrap_or_default();
    Ok(RenameOutcome {
        old: old.into(),
        new: new.into(),
        path: new_dir.display().to_string(),
        installs_with_old_name: reg.for_skill(old).len(),
    })
}

// ───────────────────────────── crear ─────────────────────────────

pub const TEMPLATES: &[&str] = &["basica", "con-scripts", "con-referencias"];

#[derive(Debug, Clone)]
pub enum NewDest {
    /// Crear la carpeta `<dir>/<name>`.
    Dir(PathBuf),
    /// Crear directamente en el banco.
    Bank,
}

#[derive(Debug, Clone)]
pub struct NewOptions {
    pub template: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub dest: NewDest,
}

#[derive(Debug, Clone, Serialize)]
pub struct NewOutcome {
    pub name: String,
    pub path: String,
    pub template: String,
    pub in_bank: bool,
    pub files: Vec<String>,
}

fn title_of(name: &str) -> String {
    let s = name.replace('-', " ");
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => s,
    }
}

/// Archivos de una plantilla ya rellenados: (ruta relativa, contenido, ejecutable).
pub fn render_template(template: &str, name: &str, description: &str) -> Result<Vec<(String, String, bool)>> {
    let title = title_of(name);
    let desc = if description.contains(':') || description.contains('#') {
        format!("\"{}\"", description.replace('"', "\\\""))
    } else {
        description.to_string()
    };
    let head = format!("---\nname: {name}\ndescription: {desc}\n---\n\n# {title}\n\n");
    let when = "## Cuándo usarla\n\n- Describe aquí las situaciones en las que el agente debe activar esta skill.\n\n";
    let files = match template {
        "basica" => vec![(
            "SKILL.md".to_string(),
            format!(
                "{head}{when}## Instrucciones\n\n1. Primer paso.\n2. Segundo paso.\n\n## Ejemplos\n\n**Entrada:** …\n\n**Salida esperada:** …\n"
            ),
            false,
        )],
        "con-scripts" => vec![
            (
                "SKILL.md".to_string(),
                format!(
                    "{head}{when}## Instrucciones\n\n1. Ejecuta `scripts/ejemplo.sh <argumento>` (ruta relativa a esta skill).\n2. Revisa la salida y continúa.\n\nEl script se ejecuta, no se lee: su contenido no ocupa contexto.\n"
                ),
                false,
            ),
            (
                "scripts/ejemplo.sh".to_string(),
                "#!/usr/bin/env bash\n# Script de ejemplo: reemplázalo por el tuyo.\nset -euo pipefail\necho \"Hola desde la skill: ${1:-sin argumento}\"\n".to_string(),
                true,
            ),
        ],
        "con-referencias" => vec![
            (
                "SKILL.md".to_string(),
                format!(
                    "{head}{when}## Instrucciones\n\n1. Sigue los pasos principales aquí.\n2. Para los detalles, lee [la guía de referencia](references/guia.md) solo cuando haga falta.\n"
                ),
                false,
            ),
            (
                "references/guia.md".to_string(),
                format!(
                    "# Guía de referencia de {title}\n\n## Contenido\n\n- [Conceptos](#conceptos)\n- [Detalles](#detalles)\n\n## Conceptos\n\n…\n\n## Detalles\n\n…\n"
                ),
                false,
            ),
        ],
        other => {
            return Err(CoreError::usage(format!("Plantilla desconocida '{other}'. Usa: {}.", TEMPLATES.join(", "))));
        }
    };
    Ok(files)
}

fn write_files(dir: &Path, files: &[(String, String, bool)]) -> Result<()> {
    for (rel, content, exec) in files {
        let p = dir.join(rel);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).at(parent)?;
        }
        fs::write(&p, content).at(&p)?;
        if *exec {
            crate::fsutil::set_executables(dir, std::slice::from_ref(rel))?;
        }
    }
    Ok(())
}

/// Crea una skill nueva desde una plantilla.
pub fn new_skill(ctx: &Context, name: &str, opts: &NewOptions) -> Result<NewOutcome> {
    let errs = name_errors(name);
    if !errs.is_empty() {
        return Err(CoreError::new(ErrorKind::Invalid, format!("'{name}' no es un nombre de skill válido."))
            .with_hint(errs.iter().filter_map(|d| d.hint.clone()).next().unwrap_or_else(|| errs[0].message.clone()))
            .with_data(json!({ "diagnostics": errs })));
    }
    let description = opts
        .description
        .clone()
        .filter(|d| !d.trim().is_empty())
        .unwrap_or_else(|| "Describe qué hace esta skill y cuándo usarla.".into());
    let files = render_template(&opts.template, name, &description)?;
    let rels: Vec<String> = files.iter().map(|f| f.0.clone()).collect();

    match &opts.dest {
        NewDest::Dir(base) => {
            let dir = base.join(name);
            if dir.exists() {
                return Err(CoreError::new(ErrorKind::Conflict, format!("Ya existe {}", dir.display())));
            }
            write_files(&dir, &files)?;
            Ok(NewOutcome {
                name: name.into(),
                path: dir.display().to_string(),
                template: opts.template.clone(),
                in_bank: false,
                files: rels,
            })
        }
        NewDest::Bank => {
            check_new_name(ctx, name)?;
            let _lock = ctx.lock_exclusive()?;
            let tmp = ctx.bank.staging_dir().join(format!("new-{}", uuid::Uuid::new_v4().simple())).join(name);
            write_files(&tmp, &files)?;
            let report = validate_dir(&tmp, &ValidateOptions::default());
            if report.has_errors() {
                let _ = fs::remove_dir_all(tmp.parent().unwrap());
                return Err(CoreError::new(ErrorKind::Invalid, "La skill generada no es válida.")
                    .with_data(json!({ "diagnostics": report.diagnostics })));
            }
            let h = tree_hash(&tmp)?;
            let stored = ctx.bank.store(name, &tmp, &h);
            let _ = fs::remove_dir_all(tmp.parent().unwrap());
            let stored = stored?;
            let mut meta = SkillMeta::new(name);
            meta.tags = normalize_tags(&opts.tags);
            meta.content_hash = h;
            meta.executables = stored.iter().filter(|f| f.executable).map(|f| f.rel.clone()).collect();
            meta.source = Some(Source { kind: "new".into(), path: None, agent: None, host: hostname() });
            ctx.bank.save_meta(&meta)?;
            crate::sync::mark_dirty(&ctx.paths.sync_state_file());
            Ok(NewOutcome {
                name: name.into(),
                path: ctx.bank.skill_dir(name).display().to_string(),
                template: opts.template.clone(),
                in_bank: true,
                files: rels,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skill::validate::Profile;

    #[test]
    fn plantillas_validas_en_todos_los_perfiles() {
        for t in TEMPLATES {
            let files = render_template(t, "mi-skill", "Hace X: útil. Úsala cuando…").unwrap();
            let td = std::env::temp_dir().join(format!("sb-tpl-{}", uuid::Uuid::new_v4().simple())).join("mi-skill");
            write_files(&td, &files).unwrap();
            for profile in [Profile::Open, Profile::ClaudeAi] {
                let r = validate_dir(&td, &ValidateOptions { profile, ..Default::default() });
                assert!(
                    r.diagnostics.iter().all(|d| d.severity == Severity::Info),
                    "{t} {profile:?}: {:?}",
                    r.diagnostics
                );
            }
            let _ = fs::remove_dir_all(td.parent().unwrap());
        }
    }

    #[test]
    fn nombres() {
        assert!(name_errors("mi-skill").is_empty());
        assert!(!name_errors("Mi Skill").is_empty());
        assert!(!name_errors("synced").is_empty());
        assert_eq!(title_of("revisar-prs"), "Revisar prs");
    }
}
