//! Registro local de instalaciones (`~/.local/state/skillbank/installs.json`).
//! Es solo local: las rutas absolutas no sirven en otra máquina. Se puede reconstruir con `scan`.

use crate::agents::{AgentId, Scope};
use crate::error::{IoContext, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InstallRecord {
    pub id: String,
    pub skill: String,
    pub scope: Scope,
    #[serde(deserialize_with = "known_agents")]
    pub agents: Vec<AgentId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_root: Option<String>,
    /// Ruta absoluta de la carpeta instalada.
    pub path: String,
    /// Hash del contenido cuando se instaló o se subió desde aquí.
    pub installed_hash: String,
    pub installed_at: DateTime<Utc>,
    /// pull | push | scan
    pub origin: String,
}

/// Lee la lista de agentes descartando los que ya no se soportan (p. ej. `codex`).
fn known_agents<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Vec<AgentId>, D::Error> {
    let raw: Vec<String> = Vec::deserialize(d)?;
    Ok(raw.iter().filter_map(|a| a.parse().ok()).collect())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Registry {
    pub schema: u32,
    #[serde(default)]
    pub installs: Vec<InstallRecord>,
    #[serde(default)]
    pub recent_projects: Vec<String>,
}

impl Default for Registry {
    fn default() -> Self {
        Registry { schema: 1, installs: vec![], recent_projects: vec![] }
    }
}

/// Estado de una instalación comparando disco (D), lo instalado (I) y el banco (B).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallStatus {
    /// D == B
    UpToDate,
    /// D == I, B != I: el banco cambió y la copia no se tocó (actualizable sin riesgo).
    Outdated,
    /// D != I, B == I: la copia local tiene cambios que el banco no tiene.
    Modified,
    /// D != I, B != I, D != B: ambos cambiaron.
    Diverged,
    /// La carpeta instalada ya no existe.
    Missing,
    /// La skill ya no está en el banco.
    Orphan,
}

impl InstallStatus {
    pub fn label_es(self) -> &'static str {
        match self {
            InstallStatus::UpToDate => "al día",
            InstallStatus::Outdated => "desactualizada",
            InstallStatus::Modified => "modificada localmente",
            InstallStatus::Diverged => "divergente",
            InstallStatus::Missing => "falta",
            InstallStatus::Orphan => "huérfana",
        }
    }

    pub fn compute(disk: Option<&str>, installed: &str, bank: Option<&str>) -> InstallStatus {
        let Some(d) = disk else { return InstallStatus::Missing };
        let Some(b) = bank else { return InstallStatus::Orphan };
        if d == b {
            InstallStatus::UpToDate
        } else if d == installed {
            InstallStatus::Outdated
        } else if b == installed {
            InstallStatus::Modified
        } else {
            InstallStatus::Diverged
        }
    }
}

impl Registry {
    pub fn load(path: &Path) -> Result<Registry> {
        if !path.exists() {
            return Ok(Registry::default());
        }
        let text = std::fs::read_to_string(path).at(path)?;
        Ok(serde_json::from_str(&text).unwrap_or_default())
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let text = serde_json::to_string_pretty(self)?;
        crate::fsutil::write_atomic(path, format!("{text}\n").as_bytes())
    }

    pub fn find_by_path(&self, path: &str) -> Option<&InstallRecord> {
        self.installs.iter().find(|r| r.path == path)
    }

    pub fn for_skill(&self, skill: &str) -> Vec<&InstallRecord> {
        self.installs.iter().filter(|r| r.skill == skill).collect()
    }

    /// Inserta o actualiza el registro de una ruta (la ruta es la identidad).
    pub fn upsert(&mut self, mut rec: InstallRecord) {
        if let Some(root) = rec.project_root.clone() {
            self.recent_projects.retain(|p| p != &root);
            self.recent_projects.insert(0, root);
            self.recent_projects.truncate(20);
        }
        if let Some(existing) = self.installs.iter_mut().find(|r| r.path == rec.path) {
            rec.id = existing.id.clone();
            for a in &existing.agents {
                if !rec.agents.contains(a) {
                    rec.agents.push(*a);
                }
            }
            rec.agents.sort();
            *existing = rec;
        } else {
            self.installs.push(rec);
        }
    }

    pub fn remove_path(&mut self, path: &str) -> bool {
        let before = self.installs.len();
        self.installs.retain(|r| r.path != path);
        before != self.installs.len()
    }
}

#[cfg(test)]
mod tests {
    use super::InstallStatus as S;

    #[test]
    fn registro_con_codex_no_se_pierde() {
        let json = r#"{"schema":1,"installs":[{"id":"1","skill":"x","scope":"project","agents":["codex","claude"],
            "path":"/p/.claude/skills/x","installed_hash":"h","installed_at":"2026-01-01T00:00:00Z","origin":"pull"}]}"#;
        let r: super::Registry = serde_json::from_str(json).unwrap();
        assert_eq!(r.installs.len(), 1);
        assert_eq!(r.installs[0].agents, vec![crate::agents::AgentId::Claude]);
    }

    #[test]
    fn matriz_de_estados() {
        assert_eq!(S::compute(None, "i", Some("b")), S::Missing);
        assert_eq!(S::compute(Some("d"), "i", None), S::Orphan);
        assert_eq!(S::compute(Some("x"), "i", Some("x")), S::UpToDate);
        assert_eq!(S::compute(Some("i"), "i", Some("b")), S::Outdated);
        assert_eq!(S::compute(Some("d"), "b", Some("b")), S::Modified);
        assert_eq!(S::compute(Some("d"), "i", Some("b")), S::Diverged);
    }
}
