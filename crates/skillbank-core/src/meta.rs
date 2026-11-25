//! Metadatos por skill en `bank/meta/<name>.json` (etiquetas, fechas, origen, hash).
//! Viven fuera de la carpeta de la skill para que esta sea idéntica a lo que se instala.

use crate::error::{IoContext, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Source {
    /// push | gui | import | new
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillMeta {
    pub schema: u32,
    pub name: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub content_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<Source>,
    /// Archivos con bit de ejecución (Drive no lo conserva; se restaura al instalar).
    #[serde(default)]
    pub executables: Vec<String>,
    #[serde(default)]
    pub notes: String,
}

impl SkillMeta {
    pub fn new(name: &str) -> SkillMeta {
        let now = Utc::now();
        SkillMeta {
            schema: 1,
            name: name.to_string(),
            tags: vec![],
            created_at: now,
            updated_at: now,
            content_hash: String::new(),
            source: None,
            executables: vec![],
            notes: String::new(),
        }
    }

    pub fn load(path: &Path) -> Result<Option<SkillMeta>> {
        if !path.exists() {
            return Ok(None);
        }
        let text = std::fs::read_to_string(path).at(path)?;
        Ok(serde_json::from_str(&text).ok())
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let text = serde_json::to_string_pretty(self)?;
        crate::fsutil::write_atomic(path, format!("{text}\n").as_bytes())
    }
}

/// Normaliza etiquetas: minúsculas, sin espacios extremos, sin duplicados, ordenadas.
pub fn normalize_tags<I: IntoIterator<Item = S>, S: AsRef<str>>(tags: I) -> Vec<String> {
    let mut v: Vec<String> = tags
        .into_iter()
        .flat_map(|t| t.as_ref().split(',').map(|s| s.trim().to_lowercase()).collect::<Vec<_>>())
        .filter(|t| !t.is_empty())
        .collect();
    v.sort();
    v.dedup();
    v
}

pub fn hostname() -> Option<String> {
    std::fs::read_to_string("/etc/hostname")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var("HOSTNAME").ok())
}
