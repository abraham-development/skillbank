//! Estado de sincronización con Google Drive (rclone bisync).
//! Fase 1: solo se marca `dirty` cuando cambia el banco. La sincronización llega en la Fase 3.

use crate::error::{IoContext, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SyncState {
    /// Hay cambios en el banco que aún no se han sincronizado.
    #[serde(default)]
    pub dirty: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_change: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_run: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_result: Option<String>,
}

impl SyncState {
    pub fn load(path: &Path) -> Result<SyncState> {
        if !path.exists() {
            return Ok(SyncState::default());
        }
        let text = std::fs::read_to_string(path).at(path)?;
        Ok(serde_json::from_str(&text).unwrap_or_default())
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        crate::fsutil::write_atomic(path, format!("{}\n", serde_json::to_string_pretty(self)?).as_bytes())
    }
}

/// Marca que el banco cambió. Si no se puede escribir (sandbox), se ignora.
pub fn mark_dirty(path: &Path) {
    let mut st = SyncState::load(path).unwrap_or_default();
    st.dirty = true;
    st.last_change = Some(Utc::now());
    let _ = st.save(path);
}
