//! `config.toml`: ajustes locales (no se sincronizan) y tabla de rutas por agente.

use crate::agents::AgentId;
use crate::error::{CoreError, IoContext, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AgentPaths {
    /// Carpeta de skills relativa a la raíz del proyecto.
    pub project_dir: String,
    /// Carpeta global de skills (admite `~`).
    pub global_dir: String,
    /// Otras carpetas de proyecto que el agente también lee.
    pub also_reads_project: Vec<String>,
    /// Otras carpetas globales que el agente también lee.
    pub also_reads_global: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Config {
    /// Carpeta del banco. Si falta, `~/.local/share/skillbank/bank`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bank_path: Option<String>,
    /// Agentes usados cuando no se indica `--agent` y no se detecta ninguno.
    pub default_agents: Vec<AgentId>,
    /// Rutas por agente (claves: claude, cursor, antigravity).
    pub agents: BTreeMap<AgentId, AgentPaths>,
}

/// Forma "cruda" del archivo: acepta agentes desconocidos (p. ej. `codex`, que ya no se
/// soporta) para ignorarlos en vez de fallar.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawConfig {
    bank_path: Option<String>,
    default_agents: Vec<String>,
    agents: BTreeMap<String, AgentPaths>,
}

impl Default for Config {
    fn default() -> Self {
        let mut agents = BTreeMap::new();
        for id in AgentId::ALL {
            agents.insert(id, default_agent_paths(id));
        }
        Config { bank_path: None, default_agents: vec![AgentId::Claude], agents }
    }
}

pub fn default_agent_paths(id: AgentId) -> AgentPaths {
    let v = |xs: &[&str]| xs.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    match id {
        AgentId::Claude => AgentPaths {
            project_dir: ".claude/skills".into(),
            global_dir: "~/.claude/skills".into(),
            also_reads_project: vec![],
            also_reads_global: vec![],
        },
        AgentId::Cursor => AgentPaths {
            project_dir: ".cursor/skills".into(),
            global_dir: "~/.cursor/skills".into(),
            also_reads_project: v(&[".agents/skills", ".claude/skills", ".codex/skills"]),
            also_reads_global: v(&["~/.agents/skills", "~/.claude/skills", "~/.codex/skills"]),
        },
        AgentId::Antigravity => AgentPaths {
            project_dir: ".agents/skills".into(),
            global_dir: "~/.gemini/config/skills".into(),
            also_reads_project: v(&[".agent/skills"]),
            also_reads_global: v(&["~/.gemini/antigravity/skills", "~/.gemini/skills"]),
        },
    }
}

impl Config {
    /// Carga `config.toml`; si no existe, devuelve los valores por defecto.
    /// Los agentes que falten en el archivo se completan con sus rutas por defecto.
    pub fn load(path: &Path) -> Result<Config> {
        let mut cfg = if path.exists() {
            let text = std::fs::read_to_string(path).at(path)?;
            let raw = toml::from_str::<RawConfig>(&text).map_err(|e| {
                CoreError::other(format!("config.toml inválido ({}): {e}", path.display()))
                    .with_hint("Corrígelo o bórralo para volver a los valores por defecto.")
            })?;
            Config {
                bank_path: raw.bank_path,
                default_agents: raw.default_agents.iter().filter_map(|a| a.parse().ok()).collect(),
                agents: raw.agents.into_iter().filter_map(|(k, v)| Some((k.parse().ok()?, v))).collect(),
            }
        } else {
            Config::default()
        };
        for id in AgentId::ALL {
            let entry = cfg.agents.entry(id).or_insert_with(|| default_agent_paths(id));
            let def = default_agent_paths(id);
            if entry.project_dir.trim().is_empty() {
                entry.project_dir = def.project_dir;
            }
            if entry.global_dir.trim().is_empty() {
                entry.global_dir = def.global_dir;
            }
        }
        if cfg.default_agents.is_empty() {
            cfg.default_agents = vec![AgentId::Claude];
        }
        Ok(cfg)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let text = toml::to_string_pretty(self).map_err(|e| CoreError::other(e.to_string()))?;
        crate::fsutil::write_atomic(path, text.as_bytes())
    }

    pub fn agent(&self, id: AgentId) -> AgentPaths {
        self.agents.get(&id).cloned().unwrap_or_else(|| default_agent_paths(id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignora_agentes_desconocidos() {
        let t = tempfile::tempdir().unwrap();
        let p = t.path().join("config.toml");
        std::fs::write(
            &p,
            "default_agents = [\"codex\", \"cursor\"]\n[agents.codex]\nproject_dir = \".agents/skills\"\n[agents.claude]\nglobal_dir = \"~/otra\"\n",
        )
        .unwrap();
        let c = Config::load(&p).unwrap();
        assert_eq!(c.default_agents, vec![AgentId::Cursor]);
        assert_eq!(c.agents.len(), 3);
        assert_eq!(c.agent(AgentId::Claude).global_dir, "~/otra");
        assert_eq!(c.agent(AgentId::Claude).project_dir, ".claude/skills");
    }
}
