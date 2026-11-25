//! Comandos que llama la interfaz. Todos son `async` y ejecutan el núcleo en un hilo aparte
//! para no bloquear la ventana. Los errores llegan a la interfaz como `CoreError` serializado
//! (`{kind, message, hint, data}`).

use crate::AppState;
use serde::Serialize;
use skillbank_core::bank::{SkillDetail, SkillSummary};
use skillbank_core::search::SearchHit;
use skillbank_core::status::InstallView;
use skillbank_core::{Context, CoreError, ErrorKind, guide, manage, search, status};
use std::path::{Path, PathBuf};
use tauri::State;

type CmdResult<T> = Result<T, CoreError>;

/// Ejecuta `f` con una copia del contexto en un hilo de bloqueo.
async fn run<T, F>(state: State<'_, AppState>, f: F) -> CmdResult<T>
where
    T: Send + 'static,
    F: FnOnce(&Context) -> CmdResult<T> + Send + 'static,
{
    let ctx = state.ctx.read().map_err(|_| CoreError::other("estado interno bloqueado"))?.clone();
    tauri::async_runtime::spawn_blocking(move || f(&ctx))
        .await
        .map_err(|e| CoreError::other(format!("tarea interrumpida: {e}")))?
}

#[derive(Serialize)]
pub struct GuideState {
    agent: String,
    name: String,
    present: bool,
    state: String,
}

#[derive(Serialize)]
pub struct AppInfo {
    version: &'static str,
    bank_path: String,
    config_path: String,
    home: String,
    skills: usize,
    cli_on_path: Option<String>,
    guide: Vec<GuideState>,
}

#[tauri::command]
pub async fn app_info(state: State<'_, AppState>) -> CmdResult<AppInfo> {
    run(state, |ctx| {
        let guide = skillbank_core::agents::AgentId::ALL
            .into_iter()
            .map(|a| GuideState {
                agent: a.key().into(),
                name: a.display().into(),
                present: a.is_present(&ctx.paths.home),
                state: guide::state_for_agent(ctx, a),
            })
            .collect();
        Ok(AppInfo {
            version: env!("CARGO_PKG_VERSION"),
            bank_path: ctx.bank.root.display().to_string(),
            config_path: ctx.paths.config_file.display().to_string(),
            home: ctx.paths.home.display().to_string(),
            skills: ctx.bank.names()?.len(),
            cli_on_path: skillbank_core::doctor::which_all("skillbank").first().map(|p| p.display().to_string()),
            guide,
        })
    })
    .await
}

#[tauri::command]
pub async fn bank_list(state: State<'_, AppState>) -> CmdResult<Vec<SkillSummary>> {
    run(state, |ctx| ctx.bank.list()).await
}

#[derive(Serialize)]
pub struct TagCount {
    tag: String,
    count: usize,
}

#[tauri::command]
pub async fn bank_tags(state: State<'_, AppState>) -> CmdResult<Vec<TagCount>> {
    run(state, |ctx| Ok(manage::all_tags(ctx)?.into_iter().map(|(tag, count)| TagCount { tag, count }).collect())).await
}

#[tauri::command]
pub async fn bank_search(state: State<'_, AppState>, query: String, tag: Option<String>) -> CmdResult<Vec<SearchHit>> {
    run(state, move |ctx| search::search(&ctx.bank, &query, tag.as_deref(), 500)).await
}

#[derive(Serialize)]
pub struct DetailView {
    #[serde(flatten)]
    detail: SkillDetail,
    installs: Vec<InstallView>,
    pull_command: String,
}

#[tauri::command]
pub async fn skill_detail(state: State<'_, AppState>, name: String) -> CmdResult<DetailView> {
    run(state, move |ctx| {
        let detail = ctx.bank.detail(&name, true)?;
        let installs = status::installed(ctx, Some(&name), None)?;
        Ok(DetailView { detail, installs, pull_command: format!("skillbank pull {name}") })
    })
    .await
}

#[derive(Serialize)]
pub struct FileContent {
    path: String,
    size: u64,
    /// Contenido si es texto UTF-8 de menos de 1 MB.
    text: Option<String>,
    binary: bool,
}

/// Lee un archivo de una skill del banco. Rechaza rutas que salgan de la carpeta de la skill.
#[tauri::command]
pub async fn skill_read_file(state: State<'_, AppState>, name: String, path: String) -> CmdResult<FileContent> {
    run(state, move |ctx| {
        let root = ctx.bank.require(&name)?;
        let rel = Path::new(&path);
        if rel.is_absolute() || rel.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
            return Err(CoreError::new(ErrorKind::Usage, "Ruta no permitida."));
        }
        let full = root.join(rel);
        let canon_root = std::fs::canonicalize(&root)?;
        let canon = std::fs::canonicalize(&full).map_err(|_| CoreError::not_found(format!("No existe {path}")))?;
        if !canon.starts_with(&canon_root) && !full.starts_with(&root) {
            return Err(CoreError::new(ErrorKind::Usage, "Ruta no permitida."));
        }
        let bytes = std::fs::read(&canon)?;
        let size = bytes.len() as u64;
        let text = if size <= 1024 * 1024 { String::from_utf8(bytes).ok() } else { None };
        Ok(FileContent { path, size, binary: text.is_none(), text })
    })
    .await
}

#[tauri::command]
pub async fn installs_list(state: State<'_, AppState>, skill: Option<String>) -> CmdResult<Vec<InstallView>> {
    run(state, move |ctx| status::installed(ctx, skill.as_deref(), None)).await
}

/// Abre una carpeta en el gestor de archivos. Solo se permiten el banco y las rutas de
/// instalaciones registradas (o su carpeta padre).
#[tauri::command]
pub async fn open_path(state: State<'_, AppState>, path: String) -> CmdResult<()> {
    run(state, move |ctx| {
        let p = PathBuf::from(&path);
        let allowed = p.starts_with(&ctx.bank.root)
            || status::installed(ctx, None, None)?
                .iter()
                .any(|v| Path::new(&v.record.path) == p || Path::new(&v.record.path).parent() == Some(p.as_path()));
        if !allowed {
            return Err(CoreError::new(
                ErrorKind::Usage,
                "Solo se pueden abrir carpetas del banco o de instalaciones.",
            ));
        }
        if !p.exists() {
            return Err(CoreError::not_found(format!("No existe {path}")));
        }
        tauri_plugin_opener::open_path(&p, None::<&str>).map_err(|e| CoreError::other(e.to_string()))
    })
    .await
}
