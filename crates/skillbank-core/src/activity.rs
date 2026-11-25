//! Conexión y actividad de los agentes (`~/.local/state/skillbank/agents.json`, solo local).
//!
//! La integración es por CLI, no por MCP, así que ninguna app de agente puede mostrar si está
//! "conectado". El CLI anota aquí cada saludo (`skillbank connect`) y cada uso identificado
//! (`--agent` o variables de entorno), y la app de escritorio lo muestra con un ícono por agente.

use crate::Context;
use crate::agents::{AgentId, Scope, primary_dir};
use crate::error::{IoContext, Result};
use crate::paths::Paths;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Máximo de eventos que se guardan en el historial.
pub const MAX_EVENTS: usize = 100;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AgentActivity {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connected_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_seen: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_skill: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_project: Option<String>,
    #[serde(default)]
    pub count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActivityEvent {
    pub at: DateTime<Utc>,
    /// Clave del agente (texto para tolerar agentes que ya no se soportan).
    pub agent: String,
    pub command: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    pub ok: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActivityLog {
    pub schema: u32,
    #[serde(default)]
    pub agents: BTreeMap<String, AgentActivity>,
    #[serde(default)]
    pub recent: Vec<ActivityEvent>,
}

impl Default for ActivityLog {
    fn default() -> Self {
        ActivityLog { schema: 1, agents: BTreeMap::new(), recent: vec![] }
    }
}

pub fn log_path(paths: &Paths) -> PathBuf {
    paths.state_dir.join("agents.json")
}

impl ActivityLog {
    pub fn load(path: &Path) -> ActivityLog {
        std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p).at(p)?;
        }
        crate::fsutil::write_atomic(path, format!("{}\n", serde_json::to_string_pretty(self)?).as_bytes())
    }

    pub fn get(&self, agent: AgentId) -> Option<&AgentActivity> {
        self.agents.get(agent.key())
    }

    /// Eventos de agentes soportados, del más reciente al más antiguo.
    pub fn events(&self) -> Vec<ActivityEvent> {
        let mut v: Vec<ActivityEvent> =
            self.recent.iter().filter(|e| e.agent.parse::<AgentId>().is_ok()).cloned().collect();
        v.reverse();
        v
    }
}

/// Un uso del CLI por parte de un agente.
#[derive(Debug, Clone, Default)]
pub struct Usage {
    pub command: String,
    pub skill: Option<String>,
    pub project: Option<String>,
    pub ok: bool,
    /// Es un saludo explícito (`skillbank connect`).
    pub connect: bool,
}

/// Anota un uso. Devuelve si era la primera conexión del agente. Si no se puede escribir
/// (p. ej. sandbox), no hace nada: la actividad nunca debe romper un comando.
pub fn record(paths: &Paths, agent: AgentId, usage: &Usage) -> Result<bool> {
    let path = log_path(paths);
    let mut log = ActivityLog::load(&path);
    let now = Utc::now();
    let entry = log.agents.entry(agent.key().to_string()).or_default();
    let first_time = entry.connected_at.is_none() && entry.last_seen.is_none();
    if usage.connect {
        entry.connected_at = Some(now);
    }
    entry.last_seen = Some(now);
    entry.last_command = Some(usage.command.clone());
    if usage.skill.is_some() {
        entry.last_skill = usage.skill.clone();
    }
    if usage.project.is_some() {
        entry.last_project = usage.project.clone();
    }
    entry.count += 1;
    log.recent.push(ActivityEvent {
        at: now,
        agent: agent.key().to_string(),
        command: usage.command.clone(),
        skill: usage.skill.clone(),
        project: usage.project.clone(),
        ok: usage.ok,
    });
    if log.recent.len() > MAX_EVENTS {
        let extra = log.recent.len() - MAX_EVENTS;
        log.recent.drain(..extra);
    }
    log.save(&path)?;
    Ok(first_time)
}

/// Olvida la conexión de un agente (su historial de eventos se conserva).
pub fn forget(paths: &Paths, agent: AgentId) -> Result<bool> {
    let path = log_path(paths);
    let mut log = ActivityLog::load(&path);
    let existed = log.agents.remove(agent.key()).is_some();
    if existed {
        log.save(&path)?;
    }
    Ok(existed)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnState {
    /// El agente no está instalado en esta máquina.
    NotInstalled,
    /// Está instalado pero le falta la skill guía (o está desactualizada).
    NeedsGuide,
    /// Tiene la guía pero aún no ha usado el CLI.
    Ready,
    /// Saludó con `skillbank connect` o ya usó el CLI.
    Connected,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentStatus {
    pub agent: AgentId,
    pub name: &'static str,
    pub present: bool,
    /// Estado de la guía: current | outdated | missing | foreign.
    pub guide: String,
    pub state: ConnState,
    #[serde(flatten)]
    pub activity: AgentActivity,
    pub project_dir: String,
    pub global_dir: String,
}

pub fn status(ctx: &Context, agent: AgentId) -> AgentStatus {
    let log = ActivityLog::load(&log_path(&ctx.paths));
    status_with(ctx, agent, &log)
}

fn status_with(ctx: &Context, agent: AgentId, log: &ActivityLog) -> AgentStatus {
    let present = agent.is_present(&ctx.paths.home);
    let guide = crate::guide::state_for_agent(ctx, agent);
    let activity = log.get(agent).cloned().unwrap_or_default();
    let used = activity.connected_at.is_some() || activity.last_seen.is_some();
    let state = if used {
        ConnState::Connected
    } else if !present {
        ConnState::NotInstalled
    } else if guide != "current" {
        ConnState::NeedsGuide
    } else {
        ConnState::Ready
    };
    let ap = ctx.config.agent(agent);
    AgentStatus {
        agent,
        name: agent.display(),
        present,
        guide,
        state,
        activity,
        project_dir: ap.project_dir.clone(),
        global_dir: primary_dir(&ap, Scope::Global, &ctx.paths.home, &ctx.paths).display().to_string(),
    }
}

pub fn all_status(ctx: &Context) -> Vec<AgentStatus> {
    let log = ActivityLog::load(&log_path(&ctx.paths));
    AgentId::ALL.into_iter().map(|a| status_with(ctx, a, &log)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(dir: &Path) -> Paths {
        Paths {
            home: dir.join("home"),
            config_file: dir.join("config.toml"),
            data_dir: dir.join("data"),
            state_dir: dir.join("state"),
            bank_override: Some(dir.join("bank")),
        }
    }

    #[test]
    fn saludo_uso_y_olvido() {
        let t = tempfile::tempdir().unwrap();
        let p = paths(t.path());
        let first = record(
            &p,
            AgentId::Claude,
            &Usage { command: "connect".into(), ok: true, connect: true, ..Default::default() },
        )
        .unwrap();
        assert!(first);
        let again = record(
            &p,
            AgentId::Claude,
            &Usage { command: "push".into(), skill: Some("x".into()), ok: true, ..Default::default() },
        )
        .unwrap();
        assert!(!again);
        let log = ActivityLog::load(&log_path(&p));
        let a = log.get(AgentId::Claude).unwrap();
        assert!(a.connected_at.is_some());
        assert_eq!(a.count, 2);
        assert_eq!(a.last_skill.as_deref(), Some("x"));
        assert_eq!(log.events()[0].command, "push");
        assert!(forget(&p, AgentId::Claude).unwrap());
        assert!(ActivityLog::load(&log_path(&p)).get(AgentId::Claude).is_none());
    }

    #[test]
    fn historial_limitado_y_tolerante() {
        let t = tempfile::tempdir().unwrap();
        let p = paths(t.path());
        for i in 0..(MAX_EVENTS + 20) {
            record(&p, AgentId::Cursor, &Usage { command: format!("c{i}"), ok: true, ..Default::default() }).unwrap();
        }
        let mut log = ActivityLog::load(&log_path(&p));
        assert_eq!(log.recent.len(), MAX_EVENTS);
        log.recent.push(ActivityEvent {
            at: Utc::now(),
            agent: "codex".into(),
            command: "push".into(),
            skill: None,
            project: None,
            ok: true,
        });
        log.save(&log_path(&p)).unwrap();
        let log = ActivityLog::load(&log_path(&p));
        assert!(log.events().iter().all(|e| e.agent == "cursor"));
    }
}
