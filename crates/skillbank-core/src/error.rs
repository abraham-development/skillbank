//! Errores del núcleo. Cada `ErrorKind` tiene un código de salida estable para el CLI
//! y un identificador estable (`code`) para la salida JSON.

use serde::Serialize;
use serde_json::Value;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorKind {
    Error,
    Usage,
    NotFound,
    Invalid,
    Conflict,
    Locked,
    SyncUnavailable,
    SyncFailed,
    Ambiguous,
}

impl ErrorKind {
    pub fn exit_code(self) -> i32 {
        match self {
            ErrorKind::Error => 1,
            ErrorKind::Usage => 2,
            ErrorKind::NotFound => 3,
            ErrorKind::Invalid => 4,
            ErrorKind::Conflict => 5,
            ErrorKind::Locked => 6,
            ErrorKind::SyncUnavailable => 7,
            ErrorKind::SyncFailed => 8,
            ErrorKind::Ambiguous => 9,
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            ErrorKind::Error => "ERROR",
            ErrorKind::Usage => "USAGE",
            ErrorKind::NotFound => "NOT_FOUND",
            ErrorKind::Invalid => "INVALID",
            ErrorKind::Conflict => "CONFLICT",
            ErrorKind::Locked => "LOCKED",
            ErrorKind::SyncUnavailable => "SYNC_UNAVAILABLE",
            ErrorKind::SyncFailed => "SYNC_FAILED",
            ErrorKind::Ambiguous => "AMBIGUOUS",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CoreError {
    pub kind: ErrorKind,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    #[serde(skip_serializing_if = "Value::is_null")]
    pub data: Value,
}

pub type Result<T> = std::result::Result<T, CoreError>;

impl CoreError {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        CoreError { kind, message: message.into(), hint: None, data: Value::Null }
    }
    pub fn other(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Error, message)
    }
    pub fn usage(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Usage, message)
    }
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::NotFound, message)
    }
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }
    pub fn with_data(mut self, data: Value) -> Self {
        self.data = data;
        self
    }
    /// Añade contexto (ruta, operación) al mensaje.
    pub fn context(mut self, ctx: impl fmt::Display) -> Self {
        self.message = format!("{ctx}: {}", self.message);
        self
    }
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for CoreError {}

impl From<std::io::Error> for CoreError {
    fn from(e: std::io::Error) -> Self {
        CoreError::other(e.to_string())
    }
}

impl From<serde_json::Error> for CoreError {
    fn from(e: serde_json::Error) -> Self {
        CoreError::other(format!("JSON inválido: {e}"))
    }
}

/// Atajo para añadir la ruta a un error de E/S.
pub trait IoContext<T> {
    fn at(self, path: &std::path::Path) -> Result<T>;
}

impl<T> IoContext<T> for std::io::Result<T> {
    fn at(self, path: &std::path::Path) -> Result<T> {
        self.map_err(|e| CoreError::other(format!("{}: {e}", path.display())))
    }
}
