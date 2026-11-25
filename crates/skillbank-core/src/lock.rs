//! Bloqueo del banco con `flock` (std::fs::File::lock, estable desde Rust 1.89).
//! Evita que la app, el CLI y la sincronización escriban a la vez.

use crate::error::{CoreError, ErrorKind, Result};
use std::fs::{File, OpenOptions, TryLockError};
use std::path::Path;
use std::thread::sleep;
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct BankLock {
    _file: File,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockMode {
    Shared,
    Exclusive,
}

impl BankLock {
    /// Adquiere el bloqueo esperando hasta `timeout`. Si la carpeta no es escribible,
    /// abre el archivo en solo lectura (flock funciona igual con descriptores de lectura).
    pub fn acquire(path: &Path, mode: LockMode, timeout: Duration) -> Result<BankLock> {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)
            .or_else(|_| File::open(path))
            .map_err(|e| CoreError::other(format!("no se pudo abrir el bloqueo {}: {e}", path.display())))?;
        let start = Instant::now();
        loop {
            let attempt = match mode {
                LockMode::Exclusive => file.try_lock(),
                LockMode::Shared => file.try_lock_shared(),
            };
            match attempt {
                Ok(()) => return Ok(BankLock { _file: file }),
                Err(TryLockError::WouldBlock) => {
                    if start.elapsed() >= timeout {
                        return Err(CoreError::new(
                            ErrorKind::Locked,
                            "El banco está ocupado (otra operación o una sincronización en curso).",
                        )
                        .with_hint("Reintenta en unos segundos o usa --lock-timeout con más segundos."));
                    }
                    sleep(Duration::from_millis(100));
                }
                Err(TryLockError::Error(e)) => {
                    return Err(CoreError::other(format!("no se pudo bloquear {}: {e}", path.display())));
                }
            }
        }
    }
}
