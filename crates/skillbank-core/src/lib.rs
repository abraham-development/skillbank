//! Núcleo de Banco de Skills: lo usan el CLI `skillbank` y la app de escritorio.

// CoreError lleva mensaje, pista y datos JSON para el agente; los errores no están en
// ningún camino caliente, así que su tamaño no importa.
#![allow(clippy::result_large_err)]

pub mod agents;
pub mod bank;
pub mod config;
pub mod diff;
pub mod error;
pub mod fsutil;
pub mod guide;
pub mod hash;
pub mod install;
pub mod lock;
pub mod meta;
pub mod paths;
pub mod registry;
pub mod search;
pub mod skill;
pub mod sync;
pub mod text;

pub use error::{CoreError, ErrorKind, Result};

use bank::Bank;
use config::Config;
use lock::{BankLock, LockMode};
use paths::Paths;
use std::time::Duration;

/// Contexto de trabajo: rutas, configuración y banco.
#[derive(Debug, Clone)]
pub struct Context {
    pub paths: Paths,
    pub config: Config,
    pub bank: Bank,
    pub lock_timeout: Duration,
}

impl Context {
    pub fn load(paths: Paths) -> Result<Context> {
        let config = Config::load(&paths.config_file)?;
        let bank_root = paths
            .bank_override
            .clone()
            .or_else(|| config.bank_path.as_deref().map(|p| paths.expand(p)))
            .unwrap_or_else(|| paths.default_bank_dir());
        Ok(Context {
            bank: Bank::new(paths::absolute(&bank_root)),
            paths,
            config,
            lock_timeout: Duration::from_secs(30),
        })
    }

    pub fn from_env() -> Result<Context> {
        Self::load(Paths::from_env())
    }

    /// Inicializa el banco si hace falta y toma el bloqueo exclusivo.
    pub fn lock_exclusive(&self) -> Result<BankLock> {
        self.bank.ensure_init()?;
        self.bank.lock(LockMode::Exclusive, self.lock_timeout)
    }
}
