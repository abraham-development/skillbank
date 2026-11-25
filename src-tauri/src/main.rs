//! App de escritorio de Banco de Skills (Tauri 2). Toda la lógica vive en `skillbank-core`;
//! aquí solo se exponen comandos a la interfaz y se vigila el banco para refrescarla.

// En release no abrir una consola extra (relevante en Windows; inocuo en Linux).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(clippy::result_large_err)]

mod commands;
mod watcher;

use std::sync::RwLock;
use tauri::Manager;

pub struct AppState {
    pub ctx: RwLock<skillbank_core::Context>,
}

fn main() {
    // Algunos drivers de GPU muestran la ventana en blanco con WebKitGTK; esta variable lo evita
    // sin coste visible. Se respeta si el usuario ya la definió.
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        // SAFETY: se ejecuta al inicio, antes de crear hilos.
        unsafe { std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1") };
    }

    let ctx = match skillbank_core::Context::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Banco de Skills: no se pudo cargar la configuración: {e}");
            std::process::exit(1);
        }
    };
    if let Err(e) = ctx.bank.ensure_init() {
        eprintln!("Banco de Skills: no se pudo preparar el banco en {}: {e}", ctx.bank.root.display());
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(AppState { ctx: RwLock::new(ctx) })
        .setup(|app| {
            watcher::start(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::bank_list,
            commands::bank_tags,
            commands::bank_search,
            commands::skill_detail,
            commands::skill_read_file,
            commands::installs_list,
            commands::open_path,
        ])
        .run(tauri::generate_context!())
        .expect("error al iniciar Banco de Skills");
}
