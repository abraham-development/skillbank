//! Hash determinista del contenido de una skill. Incluye rutas relativas y contenido;
//! excluye permisos, fechas, carpetas vacías y archivos ignorados. Así una copia
//! (o un viaje de ida y vuelta por Drive) produce el mismo hash.

use crate::error::{IoContext, Result};
use crate::fsutil::{SkillFile, list_files};
use sha2::{Digest, Sha256};
use std::path::Path;

pub fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0xf) as usize] as char);
    }
    s
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

/// Hash de un árbol a partir de su lista de archivos (ya ordenada).
pub fn hash_files(files: &[SkillFile]) -> Result<String> {
    let mut h = Sha256::new();
    for f in files {
        let bytes = std::fs::read(&f.abs).at(&f.abs)?;
        h.update(b"F\0");
        h.update(f.rel.as_bytes());
        h.update(b"\0");
        h.update((bytes.len() as u64).to_le_bytes());
        h.update(Sha256::digest(&bytes));
    }
    Ok(format!("sha256:{}", hex(&h.finalize())))
}

/// Hash del árbol de una skill.
pub fn tree_hash(root: &Path) -> Result<String> {
    hash_files(&list_files(root)?)
}

/// Versión corta para mostrar (12 caracteres hex).
pub fn short(hash: &str) -> &str {
    let h = hash.strip_prefix("sha256:").unwrap_or(hash);
    &h[..h.len().min(12)]
}
