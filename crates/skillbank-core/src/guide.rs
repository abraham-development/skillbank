//! Skill guía `banco-de-skills`: enseña a cada agente a usar el CLI. Va embebida en el binario
//! y se instala en la carpeta global de skills de cada agente.

use crate::Context;
use crate::agents::{AgentId, Scope, plan_destinations};
use crate::error::{CoreError, ErrorKind, IoContext, Result};
use crate::fsutil;
use crate::skill::SkillDoc;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

pub const GUIDE_NAME: &str = "banco-de-skills";
pub const GUIDE_VERSION: &str = "1";
const VERSION_KEY: &str = "skillbank-guide-version";

const SKILL_MD: &str = include_str!("../assets/guide/banco-de-skills/SKILL.md");
const COMMANDS_MD: &str = include_str!("../assets/guide/banco-de-skills/references/comandos.md");

/// Archivos de la guía ya renderizados (ruta relativa, contenido).
pub fn render(bin_path: &str) -> Vec<(&'static str, String)> {
    let r = |s: &str| s.replace("{{GUIDE_VERSION}}", GUIDE_VERSION).replace("{{SKILLBANK_BIN}}", bin_path);
    vec![("SKILL.md", r(SKILL_MD)), ("references/comandos.md", r(COMMANDS_MD))]
}

#[derive(Debug, Clone, Serialize)]
pub struct GuideTarget {
    pub dir: String,
    pub agents: Vec<AgentId>,
    /// missing | current | outdated | foreign
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installed_version: Option<String>,
}

fn installed_version(dir: &Path) -> Option<Option<String>> {
    let doc = SkillDoc::load(dir).ok()?;
    let v =
        doc.frontmatter.get("metadata").and_then(|m| m.get(VERSION_KEY)).and_then(|v| v.as_str()).map(str::to_string);
    Some(v)
}

fn state_of(dir: &Path) -> (String, Option<String>) {
    match installed_version(dir) {
        None => ("missing".into(), None),
        Some(None) => ("foreign".into(), None),
        Some(Some(v)) if v == GUIDE_VERSION => ("current".into(), Some(v)),
        Some(Some(v)) => ("outdated".into(), Some(v)),
    }
}

/// Estado de la guía para un agente, mirando todas las carpetas globales que lee
/// (Cursor, por ejemplo, la ve en `~/.claude/skills`). Devuelve el mejor estado encontrado.
pub fn state_for_agent(ctx: &Context, agent: AgentId) -> String {
    let rank = |s: &str| match s {
        "current" => 3,
        "outdated" => 2,
        "foreign" => 1,
        _ => 0,
    };
    crate::agents::read_dirs(&ctx.config.agent(agent), Scope::Global, &ctx.paths.home, &ctx.paths)
        .into_iter()
        .map(|d| state_of(&d.join(GUIDE_NAME)).0)
        .max_by_key(|s| rank(s))
        .unwrap_or_else(|| "missing".into())
}

/// Agentes instalados en esta máquina (por la carpeta en el HOME).
pub fn present_agents(ctx: &Context) -> Vec<AgentId> {
    AgentId::ALL.into_iter().filter(|a| a.is_present(&ctx.paths.home)).collect()
}

/// Destinos de la guía para los agentes dados (carpetas globales sin duplicados).
pub fn targets(ctx: &Context, agents: &[AgentId]) -> Vec<GuideTarget> {
    plan_destinations(agents, Scope::Global, &ctx.paths.home, &ctx.config, &ctx.paths, true)
        .into_iter()
        .map(|d| {
            let dir = d.dir.join(GUIDE_NAME);
            let (state, installed_version) = state_of(&dir);
            GuideTarget { dir: dir.display().to_string(), agents: d.agents, state, installed_version }
        })
        .collect()
}

#[derive(Debug, Clone, Serialize)]
pub struct GuideResult {
    pub dir: String,
    pub agents: Vec<AgentId>,
    /// installed | updated | unchanged | removed | absent
    pub action: String,
}

/// Instala o actualiza la guía. Si en el destino hay una skill ajena con el mismo nombre,
/// devuelve CONFLICT salvo `force`.
pub fn install(ctx: &Context, agents: &[AgentId], force: bool, bin_path: &str) -> Result<Vec<GuideResult>> {
    let files = render(bin_path);
    let mut out = Vec::new();
    let tgts = targets(ctx, agents);
    if !force && let Some(f) = tgts.iter().find(|t| t.state == "foreign") {
        return Err(CoreError::new(
            ErrorKind::Conflict,
            format!("Ya existe una skill '{GUIDE_NAME}' que no es la guía de Banco de Skills en {}.", f.dir),
        )
        .with_hint("Revísala; si quieres reemplazarla, repite con --force."));
    }
    for t in tgts {
        let dir = PathBuf::from(&t.dir);
        let action = match t.state.as_str() {
            "missing" => "installed",
            "current" if !force && same_content(&dir, &files) => "unchanged",
            _ => "updated",
        };
        if action != "unchanged" {
            let parent = dir.parent().ok_or_else(|| CoreError::other("ruta sin carpeta padre"))?;
            fs::create_dir_all(parent).at(parent)?;
            let tmp = parent.join(format!(".skillbank-tmp-guide-{}", uuid::Uuid::new_v4().simple()));
            for (rel, content) in &files {
                let p = tmp.join(rel);
                fs::create_dir_all(p.parent().unwrap()).at(&p)?;
                fs::write(&p, content).at(&p)?;
            }
            let res = fsutil::replace_dir_with_copy(&tmp, &dir);
            let _ = fs::remove_dir_all(&tmp);
            res?;
        }
        out.push(GuideResult { dir: t.dir, agents: t.agents, action: action.into() });
    }
    Ok(out)
}

fn same_content(dir: &Path, files: &[(&str, String)]) -> bool {
    files.iter().all(|(rel, c)| fs::read_to_string(dir.join(rel)).is_ok_and(|x| x == *c))
}

/// Quita la guía (solo si es nuestra).
pub fn uninstall(ctx: &Context, agents: &[AgentId]) -> Result<Vec<GuideResult>> {
    let mut out = Vec::new();
    for t in targets(ctx, agents) {
        let dir = PathBuf::from(&t.dir);
        let action = match t.state.as_str() {
            "missing" => "absent",
            "foreign" => {
                return Err(CoreError::new(
                    ErrorKind::Conflict,
                    format!("La skill en {} no es la guía de Banco de Skills; no se borró.", t.dir),
                ));
            }
            _ => {
                if fsutil::is_symlink(&dir) {
                    fs::remove_file(&dir).at(&dir)?;
                } else {
                    fs::remove_dir_all(&dir).at(&dir)?;
                }
                "removed"
            }
        };
        out.push(GuideResult { dir: t.dir, agents: t.agents, action: action.into() });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skill::validate::{ValidateOptions, validate_text};

    #[test]
    fn la_guia_es_valida_y_sin_placeholders() {
        let files = render("/usr/bin/skillbank");
        let (_, md) = &files[0];
        assert!(!md.contains("{{"));
        let rep = validate_text(md, &ValidateOptions { dir_name: Some(GUIDE_NAME.into()), ..Default::default() });
        // El nombre está reservado para el banco, pero la guía en sí es válida salvo por eso.
        let errs: Vec<_> = rep.errors().map(|d| d.code.clone()).collect();
        assert_eq!(errs, vec!["E_NAME_RESERVED".to_string()], "{:?}", rep.diagnostics);
        assert!(rep.description.unwrap().chars().count() <= 1024);
        assert!(rep.body_lines < 150);
    }
}
