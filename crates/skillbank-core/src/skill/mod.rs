//! Modelo de una skill en disco.

pub mod frontmatter;
pub mod links;
pub mod validate;

use crate::error::{CoreError, ErrorKind, IoContext, Result};
use std::path::{Path, PathBuf};
use yaml_serde::Value;

/// Contenido leído de un SKILL.md.
#[derive(Debug, Clone)]
pub struct SkillDoc {
    pub dir: PathBuf,
    pub raw: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub frontmatter: serde_json::Value,
    pub body: String,
}

impl SkillDoc {
    /// Lee el SKILL.md de una carpeta (sin validar a fondo).
    pub fn load(dir: &Path) -> Result<SkillDoc> {
        let path = dir.join("SKILL.md");
        if !path.is_file() {
            return Err(CoreError::new(ErrorKind::NotFound, format!("No hay SKILL.md en {}", dir.display())));
        }
        let raw = std::fs::read_to_string(&path).at(&path)?;
        Ok(Self::from_text(dir, raw))
    }

    pub fn from_text(dir: &Path, raw: String) -> SkillDoc {
        let sp = frontmatter::split(&raw);
        let mut name = None;
        let mut description = None;
        let mut fm_json = serde_json::Value::Null;
        if let Some(yaml) = sp.yaml.as_deref()
            && !sp.unclosed
            && let Ok(v) = frontmatter::parse_yaml(yaml, sp.yaml_first_line)
        {
            if let Value::Mapping(m) = &v {
                name = m.get(Value::String("name".into())).and_then(|v| v.as_str()).map(str::to_string);
                description = m.get(Value::String("description".into())).and_then(|v| v.as_str()).map(str::to_string);
            }
            fm_json = frontmatter::to_json(&v);
        }
        SkillDoc { dir: dir.to_path_buf(), body: sp.body, raw, name, description, frontmatter: fm_json }
    }

    /// Claves de primer nivel del frontmatter.
    pub fn keys(&self) -> Vec<String> {
        match &self.frontmatter {
            serde_json::Value::Object(m) => m.keys().cloned().collect(),
            _ => vec![],
        }
    }
}

/// Resuelve el argumento de una skill a su carpeta: acepta la carpeta o la ruta a SKILL.md.
pub fn resolve_skill_dir(p: &Path) -> Option<PathBuf> {
    if p.is_dir() && p.join("SKILL.md").is_file() {
        return Some(p.to_path_buf());
    }
    if p.is_file() && p.file_name().is_some_and(|n| n.eq_ignore_ascii_case("SKILL.md")) {
        return p.parent().map(Path::to_path_buf);
    }
    None
}
