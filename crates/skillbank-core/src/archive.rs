//! Importar y exportar skills en `.zip` (o desde carpetas).
//!
//! - Exportar una skill: `<name>.zip` con `<name>/SKILL.md…` (el formato que acepta claude.ai).
//! - Respaldo completo: `bank.json` + `skills/` + `meta/`.
//! - Importar: zip o carpeta; encuentra las skills más externas, protege contra zip-slip,
//!   enlaces simbólicos y zips gigantes, y nunca sobrescribe sin `force`.

use crate::Context;
use crate::error::{CoreError, ErrorKind, IoContext, Result};
use crate::fsutil::{self, is_ignored_name, list_files};
use crate::hash::tree_hash;
use crate::lock::LockMode;
use crate::meta::{SkillMeta, Source, hostname, normalize_tags};
use crate::skill::frontmatter::remove_top_level_keys;
use crate::skill::validate::{Diagnostic, Profile, Severity, ValidateOptions, validate_dir, validate_text};
use chrono::Utc;
use serde::Serialize;
use serde_json::json;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

/// Límites de seguridad al importar un zip.
pub const MAX_ZIP_ENTRIES: usize = 2000;
pub const MAX_ZIP_BYTES: u64 = 50 * 1024 * 1024;

// ───────────────────────────── exportar ─────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct ExportOptions {
    /// Comprueba el perfil estricto de claude.ai antes de exportar.
    pub claude_ai: bool,
    /// Quita las claves no estándar en la copia exportada (el banco no cambia).
    pub strip_nonstandard: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExportResult {
    pub name: String,
    pub path: String,
    pub files: usize,
    pub stripped_keys: Vec<String>,
    pub warnings: Vec<Diagnostic>,
}

fn file_options(executable: bool) -> SimpleFileOptions {
    use chrono::{Datelike, Local, Timelike};
    let now = Local::now();
    let mut o = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(if executable { 0o755 } else { 0o644 });
    if let Ok(dt) = zip::DateTime::from_date_and_time(
        now.year().clamp(1980, 2107) as u16,
        now.month() as u8,
        now.day() as u8,
        now.hour() as u8,
        now.minute() as u8,
        now.second().min(59) as u8,
    ) {
        o = o.last_modified_time(dt);
    }
    o
}

fn zip_err(e: zip::result::ZipError) -> CoreError {
    CoreError::other(format!("zip: {e}"))
}

/// Escribe un zip en `out` de forma atómica (temporal + rename).
fn write_zip(out: &Path, fill: impl FnOnce(&mut ZipWriter<File>) -> Result<()>) -> Result<()> {
    let parent = out.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    fs::create_dir_all(parent).at(parent)?;
    let tmp = parent.join(format!(".skillbank-tmp-{}.zip", uuid::Uuid::new_v4().simple()));
    let res = (|| {
        let f = File::create(&tmp).at(&tmp)?;
        let mut w = ZipWriter::new(f);
        fill(&mut w)?;
        w.finish().map_err(zip_err)?.sync_all().at(&tmp)?;
        fs::rename(&tmp, out).at(out)
    })();
    if res.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    res
}

/// Claves que hay que quitar para que claude.ai acepte la skill.
fn nonstandard_keys(diags: &[Diagnostic]) -> Vec<String> {
    let mut keys: Vec<String> = diags
        .iter()
        .filter(|d| d.code == "C_NONSTANDARD_KEY" || d.code == "W_ALLOWED_TOOLS_LIST")
        .filter_map(|d| d.field.clone())
        .collect();
    keys.sort();
    keys.dedup();
    keys
}

/// Exporta una skill del banco a `out` (archivo .zip).
pub fn export_skill(ctx: &Context, name: &str, out: &Path, opts: &ExportOptions) -> Result<ExportResult> {
    let dir = ctx.bank.require(name)?;
    let _lock = ctx.bank.lock(LockMode::Shared, ctx.lock_timeout)?;
    let mut skill_md = fs::read_to_string(dir.join("SKILL.md")).at(&dir.join("SKILL.md"))?;
    let profile = if opts.claude_ai { Profile::ClaudeAi } else { Profile::Open };
    let report = validate_dir(&dir, &ValidateOptions { profile, ..Default::default() });
    let mut stripped = Vec::new();
    let mut remaining: Vec<Diagnostic> = report.diagnostics.clone();

    if opts.strip_nonstandard {
        stripped = nonstandard_keys(&report.diagnostics);
        if stripped.is_empty() {
            // En perfil abierto no hay errores por claves; buscarlas igualmente.
            let strict = validate_dir(&dir, &ValidateOptions { profile: Profile::ClaudeAi, ..Default::default() });
            stripped = nonstandard_keys(&strict.diagnostics);
        }
        if !stripped.is_empty() {
            skill_md = remove_top_level_keys(&skill_md, &stripped);
            let fm = validate_text(
                &skill_md,
                &ValidateOptions { profile, dir_name: Some(name.into()), ..Default::default() },
            );
            // Diagnósticos de archivos (enlaces, tamaños) del informe original + los del nuevo frontmatter.
            remaining = report
                .diagnostics
                .iter()
                .filter(|d| d.file != "SKILL.md" || d.code.starts_with("W_BROKEN") || d.code.starts_with("W_BACKSLASH"))
                .cloned()
                .chain(fm.diagnostics)
                .collect();
        }
    }
    let errors: Vec<&Diagnostic> = remaining.iter().filter(|d| d.severity == Severity::Error).collect();
    if !errors.is_empty() {
        let has_keys = errors.iter().any(|d| d.code == "C_NONSTANDARD_KEY" || d.code == "W_ALLOWED_TOOLS_LIST");
        let mut e = CoreError::new(
            ErrorKind::Invalid,
            format!(
                "'{name}' no se puede exportar{}: {} error(es).",
                if opts.claude_ai { " para claude.ai" } else { "" },
                errors.len()
            ),
        )
        .with_data(json!({ "name": name, "diagnostics": remaining }));
        e.hint = Some(if has_keys {
            "Repite con --strip-nonstandard para quitar esos campos solo en la copia exportada.".into()
        } else {
            "Corrige la skill en el banco (o en tu proyecto y súbela) y vuelve a exportar.".into()
        });
        return Err(e);
    }

    let meta = ctx.bank.meta(name);
    let files = list_files(&dir)?;
    write_zip(out, |w| {
        w.add_directory(format!("{name}/"), file_options(true)).map_err(zip_err)?;
        for f in &files {
            let exec = f.executable || meta.executables.contains(&f.rel);
            w.start_file(format!("{name}/{}", f.rel), file_options(exec)).map_err(zip_err)?;
            if f.rel == "SKILL.md" {
                w.write_all(skill_md.as_bytes()).at(&f.abs)?;
            } else {
                w.write_all(&fs::read(&f.abs).at(&f.abs)?).at(&f.abs)?;
            }
        }
        Ok(())
    })?;
    Ok(ExportResult {
        name: name.to_string(),
        path: out.display().to_string(),
        files: files.len(),
        stripped_keys: stripped,
        warnings: remaining.into_iter().filter(|d| d.severity == Severity::Warning).collect(),
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct BackupResult {
    pub path: String,
    pub skills: usize,
}

/// Respaldo completo del banco: `bank.json`, `skills/` y `meta/`.
pub fn export_backup(ctx: &Context, out: &Path) -> Result<BackupResult> {
    if !ctx.bank.is_initialized() {
        return Err(CoreError::not_found("El banco todavía no existe: no hay nada que respaldar."));
    }
    let _lock = ctx.bank.lock(LockMode::Shared, ctx.lock_timeout)?;
    let names = ctx.bank.names()?;
    write_zip(out, |w| {
        let bank_json = ctx.bank.root.join("bank.json");
        w.start_file("bank.json", file_options(false)).map_err(zip_err)?;
        w.write_all(&fs::read(&bank_json).at(&bank_json)?).at(&bank_json)?;
        for n in &names {
            let dir = ctx.bank.skill_dir(n);
            let meta = ctx.bank.meta(n);
            for f in list_files(&dir)? {
                let exec = f.executable || meta.executables.contains(&f.rel);
                w.start_file(format!("skills/{n}/{}", f.rel), file_options(exec)).map_err(zip_err)?;
                w.write_all(&fs::read(&f.abs).at(&f.abs)?).at(&f.abs)?;
            }
            w.start_file(format!("meta/{n}.json"), file_options(false)).map_err(zip_err)?;
            w.write_all(serde_json::to_string_pretty(&meta)?.as_bytes()).at(&ctx.bank.meta_path(n))?;
        }
        Ok(())
    })?;
    Ok(BackupResult { path: out.display().to_string(), skills: names.len() })
}

// ───────────────────────────── importar ─────────────────────────────

/// Extrae un zip de forma segura en `dest` (que debe estar vacío o no existir).
pub fn safe_extract(zip_path: &Path, dest: &Path) -> Result<()> {
    let f = File::open(zip_path).at(zip_path)?;
    let mut ar = ZipArchive::new(f)
        .map_err(|e| CoreError::new(ErrorKind::Invalid, format!("{} no es un zip válido: {e}", zip_path.display())))?;
    if ar.len() > MAX_ZIP_ENTRIES {
        return Err(CoreError::new(
            ErrorKind::Invalid,
            format!("El zip tiene {} entradas (máximo {MAX_ZIP_ENTRIES}).", ar.len()),
        ));
    }
    let invalid = |m: String| CoreError::new(ErrorKind::Invalid, m);
    fs::create_dir_all(dest).at(dest)?;
    let mut total: u64 = 0;
    for i in 0..ar.len() {
        let mut entry = ar.by_index(i).map_err(zip_err)?;
        let raw_name = entry.name().map(|n| n.to_string()).unwrap_or_default();
        if entry.is_symlink() {
            return Err(invalid(format!(
                "El zip contiene un enlace simbólico ({raw_name}); no se importa por seguridad."
            )));
        }
        let Some(rel) = entry.enclosed_name() else {
            return Err(invalid(format!("Ruta peligrosa en el zip: {raw_name}")));
        };
        let skip = rel.components().any(|c| {
            let s = c.as_os_str().to_string_lossy();
            s == "__MACOSX" || s.starts_with("._")
        });
        if skip {
            continue;
        }
        let target = dest.join(&rel);
        if entry.is_dir() {
            fs::create_dir_all(&target).at(&target)?;
            continue;
        }
        total += entry.size();
        if total > MAX_ZIP_BYTES {
            return Err(invalid(format!("El zip descomprimido supera {} MB.", MAX_ZIP_BYTES / 1024 / 1024)));
        }
        if let Some(p) = target.parent() {
            fs::create_dir_all(p).at(p)?;
        }
        let mut out = File::create(&target).at(&target)?;
        let limit = MAX_ZIP_BYTES - (total - entry.size());
        let written = std::io::copy(&mut (&mut entry).take(limit + 1), &mut out).at(&target)?;
        if written > limit {
            return Err(invalid(format!("El zip descomprimido supera {} MB.", MAX_ZIP_BYTES / 1024 / 1024)));
        }
        if entry.unix_mode().is_some_and(|m| m & 0o111 != 0) {
            fsutil::set_executables(dest, &[rel.to_string_lossy().replace('\\', "/")])?;
        }
    }
    Ok(())
}

/// Carpetas de skill dentro de `root`: las que tienen SKILL.md, quedándose con las más externas.
pub fn find_skill_dirs(root: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = Vec::new();
    let walker =
        walkdir::WalkDir::new(root).follow_links(false).sort_by_file_name().into_iter().filter_entry(|e| {
            e.depth() == 0 || !is_ignored_name(&e.file_name().to_string_lossy(), e.file_type().is_dir())
        });
    for e in walker.flatten() {
        if e.file_type().is_file() && e.file_name() == "SKILL.md" {
            let dir = e.path().parent().unwrap_or(root).to_path_buf();
            if !found.iter().any(|f| dir.starts_with(f)) {
                found.push(dir);
            }
        }
    }
    found
}

#[derive(Debug, Clone, Default)]
pub struct ImportOptions {
    pub force: bool,
    pub skip_existing: bool,
    pub only: Option<Vec<String>>,
    pub tags: Option<Vec<String>>,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportRow {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Ruta relativa dentro del zip o carpeta.
    pub source: String,
    /// new | same | different | invalid | duplicate
    pub status: String,
    /// import | update | skip | conflict
    pub action: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportOutcome {
    pub source: String,
    /// zip | dir | backup
    pub kind: String,
    pub rows: Vec<ImportRow>,
    pub imported: usize,
    pub updated: usize,
    pub dry_run: bool,
}

struct Staging(Option<PathBuf>);
impl Drop for Staging {
    fn drop(&mut self) {
        if let Some(p) = &self.0 {
            let _ = fs::remove_dir_all(p);
        }
    }
}

/// Importa skills desde un zip o una carpeta.
pub fn import(ctx: &Context, path: &Path, opts: &ImportOptions) -> Result<ImportOutcome> {
    if !path.exists() {
        return Err(CoreError::not_found(format!("No existe {}", path.display())));
    }
    ctx.bank.ensure_init()?;
    let is_zip = path.is_file();
    let mut staging = Staging(None);
    let root = if is_zip {
        let tmp = ctx.bank.staging_dir().join(format!("import-{}", uuid::Uuid::new_v4().simple()));
        staging.0 = Some(tmp.clone());
        safe_extract(path, &tmp)?;
        tmp
    } else {
        path.to_path_buf()
    };

    // ¿Respaldo completo? (bank.json + skills/ en la raíz o en una única carpeta superior)
    let backup_root = [root.clone()]
        .into_iter()
        .chain(fs::read_dir(&root).ok().into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_dir()))
        .find(|r| r.join("bank.json").is_file() && r.join("skills").is_dir());
    let (kind, candidates) = match &backup_root {
        Some(b) => {
            let mut v: Vec<PathBuf> = fs::read_dir(b.join("skills"))
                .at(&b.join("skills"))?
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.join("SKILL.md").is_file())
                .collect();
            v.sort();
            ("backup", v)
        }
        None => (if is_zip { "zip" } else { "dir" }, find_skill_dirs(&root)),
    };
    if candidates.is_empty() {
        return Err(CoreError::not_found(format!(
            "No encontré ninguna skill (carpeta con SKILL.md) en {}.",
            path.display()
        )));
    }

    let mut rows = Vec::new();
    let mut plan: Vec<(usize, PathBuf, String, String)> = Vec::new(); // (fila, carpeta, nombre, hash)
    let mut seen: Vec<String> = Vec::new();
    for dir in &candidates {
        let rel = dir.strip_prefix(&root).unwrap_or(dir).display().to_string();
        let rel = if rel.is_empty() { ".".into() } else { rel };
        let report = validate_dir(dir, &ValidateOptions { dir_mismatch_is_warning: true, ..Default::default() });
        let name = report.name.clone();
        if let (Some(only), Some(n)) = (&opts.only, &name)
            && !only.contains(n)
        {
            continue;
        }
        let mut row = ImportRow {
            name: name.clone(),
            source: rel,
            status: String::new(),
            action: "skip".into(),
            diagnostics: vec![],
        };
        if report.has_errors() || name.is_none() {
            row.status = "invalid".into();
            row.diagnostics = report.errors().cloned().collect();
            rows.push(row);
            continue;
        }
        let name = name.unwrap();
        if seen.contains(&name) {
            row.status = "duplicate".into();
            rows.push(row);
            continue;
        }
        seen.push(name.clone());
        let h = tree_hash(dir)?;
        if !ctx.bank.has(&name) {
            row.status = "new".into();
            row.action = "import".into();
        } else if tree_hash(&ctx.bank.skill_dir(&name))? == h {
            row.status = "same".into();
        } else {
            row.status = "different".into();
            row.action = if opts.force {
                "update".into()
            } else if opts.skip_existing {
                "skip".into()
            } else {
                "conflict".into()
            };
        }
        if row.action == "import" || row.action == "update" {
            plan.push((rows.len(), dir.clone(), name, h));
        }
        rows.push(row);
    }

    let mut outcome = ImportOutcome {
        source: path.display().to_string(),
        kind: kind.into(),
        imported: plan.iter().filter(|(i, ..)| rows[*i].action == "import").count(),
        updated: plan.iter().filter(|(i, ..)| rows[*i].action == "update").count(),
        rows,
        dry_run: opts.dry_run,
    };
    let conflicts: Vec<&str> =
        outcome.rows.iter().filter(|r| r.action == "conflict").filter_map(|r| r.name.as_deref()).collect();
    if !conflicts.is_empty() {
        return Err(CoreError::new(
            ErrorKind::Conflict,
            format!("No se importó nada: ya existen versiones distintas de {}.", conflicts.join(", ")),
        )
        .with_hint("Usa --skip-existing para importar solo las nuevas, o --force (con permiso) para sobrescribir.")
        .with_data(serde_json::to_value(&outcome)?));
    }
    if opts.dry_run || plan.is_empty() {
        return Ok(outcome);
    }

    let _lock = ctx.lock_exclusive()?;
    for (_, dir, name, h) in &plan {
        let files = ctx.bank.store(name, dir, h)?;
        let mut meta = if ctx.bank.has(name) && ctx.bank.meta_path(name).exists() {
            ctx.bank.meta(name)
        } else {
            SkillMeta::new(name)
        };
        if let Some(b) = &backup_root
            && let Ok(Some(m)) = SkillMeta::load(&b.join("meta").join(format!("{name}.json")))
        {
            meta.tags = m.tags;
            meta.created_at = m.created_at;
            meta.notes = m.notes;
        }
        if let Some(t) = &opts.tags {
            meta.tags = normalize_tags(meta.tags.iter().cloned().chain(t.iter().cloned()));
        }
        meta.name = name.clone();
        meta.updated_at = Utc::now();
        meta.content_hash = h.clone();
        meta.executables = files.iter().filter(|f| f.executable).map(|f| f.rel.clone()).collect();
        meta.source = Some(Source {
            kind: "import".into(),
            path: Some(path.display().to_string()),
            agent: None,
            host: hostname(),
        });
        ctx.bank.save_meta(&meta)?;
    }
    crate::sync::mark_dirty(&ctx.paths.sync_state_file());
    ctx.bank.purge_trash();
    outcome.dry_run = false;
    Ok(outcome)
}
