//! El banco en disco:
//! ```text
//! <bank>/bank.json  RCLONE_TEST
//! <bank>/skills/<name>/…        carpetas de skill idénticas a lo que se instala
//! <bank>/meta/<name>.json       metadatos (etiquetas, fechas, origen)
//! <bank>/.skillbank/            interno, no se sincroniza: bank.lock, staging/, trash/
//! ```

use crate::error::{CoreError, ErrorKind, IoContext, Result};
use crate::fsutil::{self, SkillFile};
use crate::hash;
use crate::lock::{BankLock, LockMode};
use crate::meta::SkillMeta;
use crate::skill::SkillDoc;
use crate::skill::validate::{Diagnostic, ValidateOptions, ValidationReport, validate_dir};
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const TRASH_DAYS: u64 = 7;

#[derive(Debug, Clone)]
pub struct Bank {
    pub root: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct SkillSummary {
    pub name: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub content_hash: String,
    pub valid: bool,
    pub errors: usize,
    pub warnings: usize,
    pub claude_code_fields: Vec<String>,
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileInfo {
    pub path: String,
    pub size: u64,
    pub executable: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SkillDetail {
    #[serde(flatten)]
    pub summary: SkillSummary,
    pub frontmatter: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    pub files: Vec<FileInfo>,
    pub diagnostics: Vec<Diagnostic>,
    pub meta: SkillMeta,
}

impl Bank {
    pub fn new(root: impl Into<PathBuf>) -> Bank {
        Bank { root: root.into() }
    }

    pub fn skills_dir(&self) -> PathBuf {
        self.root.join("skills")
    }
    pub fn meta_dir(&self) -> PathBuf {
        self.root.join("meta")
    }
    pub fn internal_dir(&self) -> PathBuf {
        self.root.join(".skillbank")
    }
    pub fn trash_dir(&self) -> PathBuf {
        self.internal_dir().join("trash")
    }
    pub fn staging_dir(&self) -> PathBuf {
        self.internal_dir().join("staging")
    }
    pub fn lock_path(&self) -> PathBuf {
        self.internal_dir().join("bank.lock")
    }
    pub fn skill_dir(&self, name: &str) -> PathBuf {
        self.skills_dir().join(name)
    }
    pub fn meta_path(&self, name: &str) -> PathBuf {
        self.meta_dir().join(format!("{name}.json"))
    }

    pub fn is_initialized(&self) -> bool {
        self.root.join("bank.json").is_file()
    }

    /// Crea la estructura del banco si falta (idempotente).
    pub fn ensure_init(&self) -> Result<()> {
        for d in [self.skills_dir(), self.meta_dir(), self.staging_dir(), self.trash_dir()] {
            fs::create_dir_all(&d).at(&d)?;
        }
        let bank_json = self.root.join("bank.json");
        if !bank_json.exists() {
            let v = serde_json::json!({
                "format": 1,
                "id": uuid::Uuid::new_v4().to_string(),
                "created_at": Utc::now(),
                "app": "Banco de Skills"
            });
            fsutil::write_atomic(&bank_json, format!("{}\n", serde_json::to_string_pretty(&v)?).as_bytes())?;
        }
        let rclone_test = self.root.join("RCLONE_TEST");
        if !rclone_test.exists() {
            fsutil::write_atomic(&rclone_test, b"Archivo de control de Banco de Skills para rclone bisync.\n")?;
        }
        Ok(())
    }

    pub fn lock(&self, mode: LockMode, timeout: Duration) -> Result<BankLock> {
        fs::create_dir_all(self.internal_dir()).at(&self.internal_dir())?;
        BankLock::acquire(&self.lock_path(), mode, timeout)
    }

    /// Nombres de las skills del banco, ordenados.
    pub fn names(&self) -> Result<Vec<String>> {
        let dir = self.skills_dir();
        if !dir.is_dir() {
            return Ok(vec![]);
        }
        let mut v = Vec::new();
        for e in fs::read_dir(&dir).at(&dir)? {
            let e = e.at(&dir)?;
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') || !e.path().is_dir() {
                continue;
            }
            v.push(name);
        }
        v.sort();
        Ok(v)
    }

    pub fn has(&self, name: &str) -> bool {
        self.skill_dir(name).join("SKILL.md").is_file()
    }

    pub fn require(&self, name: &str) -> Result<PathBuf> {
        if self.has(name) {
            return Ok(self.skill_dir(name));
        }
        let suggestions = self.did_you_mean(name);
        let mut err = CoreError::new(ErrorKind::NotFound, format!("La skill '{name}' no está en el banco."))
            .with_data(serde_json::json!({ "name": name, "did_you_mean": suggestions }));
        err.hint = Some(if suggestions.is_empty() {
            "Usa `skillbank list` o `skillbank search <texto>` para ver lo que hay.".into()
        } else {
            format!("¿Quisiste decir: {}?", suggestions.join(", "))
        });
        Err(err)
    }

    pub fn did_you_mean(&self, name: &str) -> Vec<String> {
        let q = crate::text::fold(name);
        let mut scored: Vec<(usize, String)> = self
            .names()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|n| {
                let d = crate::text::edit_distance(&q, &n);
                if d <= 3 || n.contains(&q) || (q.len() >= 3 && q.contains(&n)) { Some((d, n)) } else { None }
            })
            .collect();
        scored.sort();
        scored.into_iter().take(5).map(|(_, n)| n).collect()
    }

    /// Metadatos de una skill; si faltan se reconstruyen a partir de la carpeta.
    pub fn meta(&self, name: &str) -> SkillMeta {
        if let Ok(Some(m)) = SkillMeta::load(&self.meta_path(name)) {
            return m;
        }
        let mut m = SkillMeta::new(name);
        if let Ok(md) = fs::metadata(self.skill_dir(name))
            && let Ok(t) = md.modified()
        {
            let dt: DateTime<Utc> = t.into();
            m.created_at = dt;
            m.updated_at = dt;
        }
        m
    }

    pub fn save_meta(&self, meta: &SkillMeta) -> Result<()> {
        meta.save(&self.meta_path(&meta.name))
    }

    pub fn validate(&self, name: &str) -> ValidationReport {
        validate_dir(&self.skill_dir(name), &ValidateOptions::default())
    }

    pub fn summary(&self, name: &str) -> Result<SkillSummary> {
        let dir = self.require(name)?;
        let doc = SkillDoc::load(&dir)?;
        let meta = self.meta(name);
        let report = self.validate(name);
        let content_hash =
            if meta.content_hash.is_empty() { hash::tree_hash(&dir)? } else { meta.content_hash.clone() };
        Ok(SkillSummary {
            name: name.to_string(),
            description: doc.description,
            tags: meta.tags.clone(),
            created_at: meta.created_at,
            updated_at: meta.updated_at,
            content_hash,
            valid: !report.has_errors(),
            errors: report.count(crate::skill::validate::Severity::Error),
            warnings: report.count(crate::skill::validate::Severity::Warning),
            claude_code_fields: report.claude_code_fields,
            path: dir.display().to_string(),
        })
    }

    pub fn list(&self) -> Result<Vec<SkillSummary>> {
        let mut out = Vec::new();
        for n in self.names()? {
            if let Ok(s) = self.summary(&n) {
                out.push(s);
            }
        }
        Ok(out)
    }

    pub fn detail(&self, name: &str, with_body: bool) -> Result<SkillDetail> {
        let dir = self.require(name)?;
        let doc = SkillDoc::load(&dir)?;
        let mut summary = self.summary(name)?;
        summary.content_hash = hash::tree_hash(&dir)?;
        let files = fsutil::list_files(&dir)?
            .into_iter()
            .map(|f| FileInfo { path: f.rel, size: f.size, executable: f.executable })
            .collect();
        let report = self.validate(name);
        Ok(SkillDetail {
            summary,
            frontmatter: doc.frontmatter,
            body: with_body.then_some(doc.body),
            files,
            diagnostics: report.diagnostics,
            meta: self.meta(name),
        })
    }

    /// Copia `src` al banco como `name`, reemplazando de forma atómica la versión anterior
    /// (que va a la papelera). El llamador debe tener el bloqueo exclusivo.
    pub fn store(&self, name: &str, src: &Path, expected_hash: &str) -> Result<Vec<SkillFile>> {
        self.ensure_init()?;
        let stage_root = self.staging_dir().join(uuid::Uuid::new_v4().simple().to_string());
        let staged = stage_root.join(name);
        let result = (|| {
            let files = fsutil::copy_tree(src, &staged)?;
            let got = hash::hash_files(&fsutil::list_files(&staged)?)?;
            if got != expected_hash {
                return Err(CoreError::other(
                    "La skill cambió mientras se copiaba; vuelve a intentarlo cuando nadie la esté editando.",
                ));
            }
            let target = self.skill_dir(name);
            if target.exists() {
                self.move_to_trash(name)?;
            }
            fs::rename(&staged, &target).at(&target)?;
            Ok(files)
        })();
        let _ = fs::remove_dir_all(&stage_root);
        result
    }

    /// Mueve una skill (y sus metadatos) a la papelera local. Devuelve la ruta en la papelera.
    pub fn move_to_trash(&self, name: &str) -> Result<PathBuf> {
        let trash = self.trash_dir();
        fs::create_dir_all(&trash).at(&trash)?;
        let stamp = Utc::now().format("%Y%m%d-%H%M%S");
        let dest = trash.join(format!("{name}-{stamp}-{}", &uuid::Uuid::new_v4().simple().to_string()[..6]));
        let src = self.skill_dir(name);
        if src.exists() {
            fs::rename(&src, &dest).at(&src)?;
        }
        let meta = self.meta_path(name);
        if meta.exists() {
            let _ = fs::copy(&meta, dest.with_extension("meta.json"));
        }
        Ok(dest)
    }

    /// Elimina una skill del banco (la mueve a la papelera). El llamador tiene el bloqueo.
    pub fn remove(&self, name: &str) -> Result<PathBuf> {
        self.require(name)?;
        let dest = self.move_to_trash(name)?;
        let _ = fs::remove_file(self.meta_path(name));
        Ok(dest)
    }

    /// Borra de la papelera lo que tenga más de `TRASH_DAYS` días.
    pub fn purge_trash(&self) {
        let Ok(entries) = fs::read_dir(self.trash_dir()) else { return };
        let limit = Duration::from_secs(TRASH_DAYS * 24 * 3600);
        for e in entries.flatten() {
            let old = e
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.elapsed().ok())
                .is_some_and(|age| age > limit);
            if old {
                let p = e.path();
                if p.is_dir() {
                    let _ = fs::remove_dir_all(&p);
                } else {
                    let _ = fs::remove_file(&p);
                }
            }
        }
    }
}
