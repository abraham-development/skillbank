//! Agentes destino (Claude Code, Cursor, Antigravity): detección y plan de
//! carpetas de instalación sin duplicados.

use crate::config::{AgentPaths, Config};
use crate::paths::{Paths, normalize};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentId {
    Claude,
    Cursor,
    Antigravity,
}

impl AgentId {
    pub const ALL: [AgentId; 3] = [AgentId::Claude, AgentId::Cursor, AgentId::Antigravity];

    pub fn key(self) -> &'static str {
        match self {
            AgentId::Claude => "claude",
            AgentId::Cursor => "cursor",
            AgentId::Antigravity => "antigravity",
        }
    }

    pub fn display(self) -> &'static str {
        match self {
            AgentId::Claude => "Claude Code",
            AgentId::Cursor => "Cursor",
            AgentId::Antigravity => "Antigravity",
        }
    }

    /// Campos del frontmatter fuera del estándar que este agente sí entiende.
    pub fn understands_field(self, key: &str) -> bool {
        match self {
            AgentId::Claude => true,
            AgentId::Cursor => matches!(key, "paths" | "disable-model-invocation" | "icon" | "color"),
            AgentId::Antigravity => false,
        }
    }

    /// Marcadores en la raíz de un proyecto que indican que se usa este agente.
    fn project_markers(self) -> &'static [&'static str] {
        match self {
            AgentId::Claude => &[".claude", "CLAUDE.md"],
            AgentId::Cursor => &[".cursor", ".cursorrules"],
            AgentId::Antigravity => &[".agents", ".agent", "GEMINI.md"],
        }
    }

    /// Carpeta del HOME cuya existencia indica que el agente está instalado.
    fn presence_dirs(self) -> &'static [&'static str] {
        match self {
            AgentId::Claude => &[".claude"],
            AgentId::Cursor => &[".cursor"],
            AgentId::Antigravity => &[".gemini"],
        }
    }

    pub fn is_present(self, home: &Path) -> bool {
        self.presence_dirs().iter().any(|d| home.join(d).is_dir())
    }
}

impl std::fmt::Display for AgentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.key())
    }
}

impl std::str::FromStr for AgentId {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "claude" | "claude-code" | "claudecode" => Ok(AgentId::Claude),
            "codex" | "openai-codex" => Err("Codex ya no está soportado; usa claude, cursor o antigravity".to_string()),
            "cursor" => Ok(AgentId::Cursor),
            "antigravity" | "gemini" | "google-antigravity" => Ok(AgentId::Antigravity),
            other => Err(format!("agente desconocido '{other}' (usa claude, cursor o antigravity)")),
        }
    }
}

/// Cómo se eligieron los agentes (se informa en la salida).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentSource {
    Explicit,
    Environment,
    ProjectMarkers,
    ConfigDefault,
    All,
}

/// Selección pedida por el usuario: lista explícita, `all` o `detected`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentRequest {
    Auto,
    All,
    Detected,
    List(Vec<AgentId>),
}

impl AgentRequest {
    /// Interpreta los valores de `--agent` (repetible; admite `all` y `detected`).
    pub fn parse(values: &[String]) -> Result<AgentRequest, String> {
        if values.is_empty() {
            return Ok(AgentRequest::Auto);
        }
        let mut list = Vec::new();
        for v in values {
            for part in v.split(',').map(str::trim).filter(|p| !p.is_empty()) {
                match part.to_lowercase().as_str() {
                    "all" | "todos" => return Ok(AgentRequest::All),
                    "detected" | "detectados" => return Ok(AgentRequest::Detected),
                    _ => {
                        let id: AgentId = part.parse()?;
                        if !list.contains(&id) {
                            list.push(id);
                        }
                    }
                }
            }
        }
        Ok(AgentRequest::List(list))
    }
}

/// Agentes detectados por marcadores en la raíz del proyecto.
pub fn detect_project_agents(project_root: &Path) -> Vec<AgentId> {
    AgentId::ALL.into_iter().filter(|a| a.project_markers().iter().any(|m| project_root.join(m).exists())).collect()
}

/// Agente que está ejecutando el CLI, si se reconoce por variables de entorno.
pub fn agent_from_env() -> Option<AgentId> {
    // CLAUDECODE=1 lo pone Claude Code en sus comandos; CURSOR_AGENT lo documenta Cursor.
    // Antigravity no tiene una variable documentada: depende de `--agent`.
    if std::env::var_os("CLAUDECODE").is_some_and(|v| v == "1") {
        return Some(AgentId::Claude);
    }
    if std::env::var_os("CURSOR_AGENT").is_some_and(|v| !v.is_empty()) {
        return Some(AgentId::Cursor);
    }
    None
}

/// Resuelve la lista final de agentes.
pub fn resolve_agents(req: &AgentRequest, project_root: Option<&Path>, cfg: &Config) -> (Vec<AgentId>, AgentSource) {
    match req {
        AgentRequest::List(l) if !l.is_empty() => (l.clone(), AgentSource::Explicit),
        AgentRequest::All => (AgentId::ALL.to_vec(), AgentSource::All),
        AgentRequest::Detected => {
            let d = project_root.map(detect_project_agents).unwrap_or_default();
            if d.is_empty() {
                (cfg.default_agents.clone(), AgentSource::ConfigDefault)
            } else {
                (d, AgentSource::ProjectMarkers)
            }
        }
        _ => {
            if let Some(a) = agent_from_env() {
                return (vec![a], AgentSource::Environment);
            }
            let d = project_root.map(detect_project_agents).unwrap_or_default();
            if !d.is_empty() {
                return (d, AgentSource::ProjectMarkers);
            }
            (cfg.default_agents.clone(), AgentSource::ConfigDefault)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    Project,
    Global,
}

/// Carpeta de skills de destino y los agentes que la leerán.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Destination {
    pub dir: PathBuf,
    pub agents: Vec<AgentId>,
}

fn resolve_dir(base: &Path, rel_or_tilde: &str, paths: &Paths) -> PathBuf {
    let p = paths.expand(rel_or_tilde);
    if p.is_absolute() { normalize(&p) } else { normalize(&base.join(p)) }
}

/// Carpeta principal de un agente para el ámbito dado.
pub fn primary_dir(ap: &AgentPaths, scope: Scope, project_root: &Path, paths: &Paths) -> PathBuf {
    match scope {
        Scope::Project => resolve_dir(project_root, &ap.project_dir, paths),
        Scope::Global => resolve_dir(&paths.home, &ap.global_dir, paths),
    }
}

/// Todas las carpetas que lee un agente en un ámbito (principal + adicionales).
pub fn read_dirs(ap: &AgentPaths, scope: Scope, project_root: &Path, paths: &Paths) -> Vec<PathBuf> {
    let mut v = vec![primary_dir(ap, scope, project_root, paths)];
    let extra = match scope {
        Scope::Project => &ap.also_reads_project,
        Scope::Global => &ap.also_reads_global,
    };
    let base = match scope {
        Scope::Project => project_root.to_path_buf(),
        Scope::Global => paths.home.clone(),
    };
    for e in extra {
        let d = resolve_dir(&base, e, paths);
        if !v.contains(&d) {
            v.push(d);
        }
    }
    v
}

/// Plan de carpetas de destino. Con `dedupe`, elige el menor conjunto de carpetas principales
/// tal que cada agente lea al menos una (así Cursor no ve la misma skill dos veces).
pub fn plan_destinations(
    agents: &[AgentId],
    scope: Scope,
    project_root: &Path,
    cfg: &Config,
    paths: &Paths,
    dedupe: bool,
) -> Vec<Destination> {
    // Candidatas: carpetas principales en orden, sin repetir.
    let mut candidates: Vec<PathBuf> = Vec::new();
    for a in agents {
        let d = primary_dir(&cfg.agent(*a), scope, project_root, paths);
        if !candidates.contains(&d) {
            candidates.push(d);
        }
    }
    let reads: Vec<(AgentId, Vec<PathBuf>)> =
        agents.iter().map(|a| (*a, read_dirs(&cfg.agent(*a), scope, project_root, paths))).collect();

    let chosen: Vec<PathBuf> = if dedupe && candidates.len() > 1 && candidates.len() <= 16 {
        let n = candidates.len();
        let mut best: Option<Vec<usize>> = None;
        // Subconjuntos por tamaño creciente; el primero que cubre (en orden) gana.
        'outer: for size in 1..=n {
            for mask in 0u32..(1 << n) {
                if mask.count_ones() as usize != size {
                    continue;
                }
                let idx: Vec<usize> = (0..n).filter(|i| mask & (1 << i) != 0).collect();
                let covers = reads.iter().all(|(_, rd)| idx.iter().any(|i| rd.contains(&candidates[*i])));
                if covers {
                    best = Some(idx);
                    break 'outer;
                }
            }
        }
        best.map(|idx| idx.into_iter().map(|i| candidates[i].clone()).collect()).unwrap_or(candidates.clone())
    } else {
        candidates.clone()
    };

    chosen
        .into_iter()
        .map(|dir| {
            let covered: Vec<AgentId> = reads
                .iter()
                .filter(|(a, rd)| {
                    if dedupe {
                        rd.contains(&dir)
                    } else {
                        primary_dir(&cfg.agent(*a), scope, project_root, paths) == dir
                    }
                })
                .map(|(a, _)| *a)
                .collect();
            Destination { dir, agents: covered }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(home: &str) -> Paths {
        Paths {
            home: PathBuf::from(home),
            config_file: PathBuf::from("/nope/config.toml"),
            data_dir: PathBuf::from("/nope/data"),
            state_dir: PathBuf::from("/nope/state"),
            bank_override: None,
        }
    }

    fn dirs(d: &[Destination]) -> Vec<String> {
        d.iter().map(|x| x.dir.display().to_string()).collect()
    }

    #[test]
    fn dedupe_claude_cursor_una_carpeta() {
        let cfg = Config::default();
        let p = paths("/h");
        let plan =
            plan_destinations(&[AgentId::Claude, AgentId::Cursor], Scope::Project, Path::new("/p"), &cfg, &p, true);
        assert_eq!(dirs(&plan), vec!["/p/.claude/skills"]);
        assert_eq!(plan[0].agents, vec![AgentId::Claude, AgentId::Cursor]);
    }

    #[test]
    fn antigravity_y_cursor_una_carpeta() {
        let cfg = Config::default();
        let p = paths("/h");
        let plan = plan_destinations(
            &[AgentId::Antigravity, AgentId::Cursor],
            Scope::Project,
            Path::new("/p"),
            &cfg,
            &p,
            true,
        );
        assert_eq!(dirs(&plan), vec!["/p/.agents/skills"]);
        assert_eq!(plan[0].agents, vec![AgentId::Antigravity, AgentId::Cursor]);
    }

    #[test]
    fn proyecto_los_tres_dos_carpetas() {
        let cfg = Config::default();
        let p = paths("/h");
        let plan = plan_destinations(&AgentId::ALL, Scope::Project, Path::new("/p"), &cfg, &p, true);
        assert_eq!(dirs(&plan), vec!["/p/.claude/skills", "/p/.agents/skills"]);
        // Cursor lee las dos: se avisará del duplicado.
        assert!(plan.iter().all(|d| d.agents.contains(&AgentId::Cursor)));
    }

    #[test]
    fn global_los_tres_dos_carpetas() {
        let cfg = Config::default();
        let p = paths("/h");
        let plan = plan_destinations(&AgentId::ALL, Scope::Global, Path::new("/p"), &cfg, &p, true);
        assert_eq!(dirs(&plan), vec!["/h/.claude/skills", "/h/.gemini/config/skills"]);
        assert_eq!(plan[0].agents, vec![AgentId::Claude, AgentId::Cursor]);
        assert_eq!(plan[1].agents, vec![AgentId::Antigravity]);
    }

    #[test]
    fn codex_ya_no_se_acepta() {
        let e = AgentRequest::parse(&["codex".into()]).unwrap_err();
        assert!(e.contains("Codex ya no está soportado"));
    }

    #[test]
    fn sin_dedupe_todas_las_principales() {
        let cfg = Config::default();
        let p = paths("/h");
        let plan =
            plan_destinations(&[AgentId::Claude, AgentId::Cursor], Scope::Project, Path::new("/p"), &cfg, &p, false);
        assert_eq!(dirs(&plan), vec!["/p/.claude/skills", "/p/.cursor/skills"]);
        assert_eq!(plan[1].agents, vec![AgentId::Cursor]);
    }

    #[test]
    fn parse_peticion() {
        assert_eq!(AgentRequest::parse(&[]).unwrap(), AgentRequest::Auto);
        assert_eq!(AgentRequest::parse(&["all".into()]).unwrap(), AgentRequest::All);
        assert_eq!(
            AgentRequest::parse(&["claude,cursor".into(), "claude".into()]).unwrap(),
            AgentRequest::List(vec![AgentId::Claude, AgentId::Cursor])
        );
        assert!(AgentRequest::parse(&["vim".into()]).is_err());
    }

    #[test]
    fn detecta_marcadores() {
        let t = tempfile::tempdir().unwrap();
        std::fs::create_dir(t.path().join(".cursor")).unwrap();
        std::fs::write(t.path().join("CLAUDE.md"), "x").unwrap();
        assert_eq!(detect_project_agents(t.path()), vec![AgentId::Claude, AgentId::Cursor]);
    }
}
