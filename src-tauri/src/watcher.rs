//! Vigila el banco y el registro de instalaciones: cuando un agente sube o trae una skill con
//! el CLI, la interfaz se entera (eventos `bank-changed` e `installs-changed`) y se refresca sola.

use crate::AppState;
use notify_debouncer_full::notify::RecursiveMode;
use notify_debouncer_full::{DebounceEventResult, new_debouncer};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

pub fn start(app: AppHandle) {
    let (bank_root, state_dir) = {
        let st = app.state::<AppState>();
        let ctx = st.ctx.read().expect("estado");
        (ctx.bank.root.clone(), ctx.paths.state_dir.clone())
    };
    let _ = std::fs::create_dir_all(&state_dir);
    let internal = bank_root.join(".skillbank");
    let registry = state_dir.join("installs.json");
    let handle = app.clone();

    let debouncer = new_debouncer(Duration::from_millis(300), None, move |res: DebounceEventResult| {
        let Ok(events) = res else { return };
        let mut bank = false;
        let mut installs = false;
        for ev in &events {
            for p in &ev.paths {
                if p.starts_with(&internal) {
                    continue; // bloqueo, staging y papelera: ruido
                }
                if p.starts_with(&bank_root) {
                    bank = true;
                }
                if *p == registry {
                    installs = true;
                }
            }
        }
        let debug = std::env::var_os("SKILLBANK_DEBUG").is_some();
        if bank {
            let _ = handle.emit("bank-changed", ());
            if debug {
                eprintln!("[skillbank] evento: bank-changed");
            }
        }
        if installs {
            let _ = handle.emit("installs-changed", ());
            if debug {
                eprintln!("[skillbank] evento: installs-changed");
            }
        }
    });

    match debouncer {
        Ok(mut d) => {
            let root = app.state::<AppState>().ctx.read().expect("estado").bank.root.clone();
            if let Err(e) = d.watch(&root, RecursiveMode::Recursive) {
                eprintln!("Banco de Skills: no se puede vigilar el banco: {e}");
            }
            if let Err(e) = d.watch(&state_dir, RecursiveMode::NonRecursive) {
                eprintln!("Banco de Skills: no se puede vigilar el registro: {e}");
            }
            // Mantener vivo el vigilante mientras viva la app.
            app.manage(WatcherHandle(std::sync::Mutex::new(Some(Box::new(d)))));
        }
        Err(e) => eprintln!("Banco de Skills: vigilante no disponible: {e}"),
    }
}

pub struct WatcherHandle(#[allow(dead_code)] std::sync::Mutex<Option<Box<dyn std::any::Any + Send>>>);
