//! Resolución de rutas (XDG en Linux) con overrides por variables de entorno
//! (`SKILLBANK_CONFIG`, `SKILLBANK_BANK`, `SKILLBANK_STATE_DIR`) para pruebas herméticas.

use std::env;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Paths {
    pub home: PathBuf,
    pub config_file: PathBuf,
    pub data_dir: PathBuf,
    pub state_dir: PathBuf,
    /// Banco forzado por `--bank` o `SKILLBANK_BANK` (tiene prioridad sobre config.toml).
    pub bank_override: Option<PathBuf>,
}

fn xdg(var: &str, home: &Path, default_rel: &str) -> PathBuf {
    match env::var_os(var) {
        Some(v) if !v.is_empty() && Path::new(&v).is_absolute() => PathBuf::from(v),
        _ => home.join(default_rel),
    }
}

pub fn home_dir() -> PathBuf {
    env::var_os("HOME").filter(|h| !h.is_empty()).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/"))
}

impl Paths {
    /// Rutas por defecto según el entorno actual.
    pub fn from_env() -> Self {
        let home = home_dir();
        let config_file = env::var_os("SKILLBANK_CONFIG")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| xdg("XDG_CONFIG_HOME", &home, ".config").join("skillbank").join("config.toml"));
        let data_dir = xdg("XDG_DATA_HOME", &home, ".local/share").join("skillbank");
        let state_dir = env::var_os("SKILLBANK_STATE_DIR")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| xdg("XDG_STATE_HOME", &home, ".local/state").join("skillbank"));
        let bank_override = env::var_os("SKILLBANK_BANK").filter(|v| !v.is_empty()).map(PathBuf::from);
        Paths { home, config_file, data_dir, state_dir, bank_override }
    }

    pub fn default_bank_dir(&self) -> PathBuf {
        self.data_dir.join("bank")
    }

    pub fn registry_file(&self) -> PathBuf {
        self.state_dir.join("installs.json")
    }

    pub fn sync_state_file(&self) -> PathBuf {
        self.state_dir.join("sync.json")
    }

    /// Expande `~` y `~/` con el HOME de estas rutas.
    pub fn expand(&self, p: &str) -> PathBuf {
        expand_tilde(p, &self.home)
    }
}

pub fn expand_tilde(p: &str, home: &Path) -> PathBuf {
    if p == "~" {
        home.to_path_buf()
    } else if let Some(rest) = p.strip_prefix("~/") {
        home.join(rest)
    } else {
        PathBuf::from(p)
    }
}

/// Normaliza `.` y `..` sin tocar el disco (no resuelve symlinks).
pub fn normalize(p: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Ruta absoluta (relativa al cwd si hace falta) y normalizada.
pub fn absolute(p: &Path) -> PathBuf {
    if p.is_absolute() {
        normalize(p)
    } else {
        let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
        normalize(&cwd.join(p))
    }
}

/// Raíz del proyecto: el directorio más cercano hacia arriba que contenga `.git`; si no hay, `start`.
pub fn project_root(start: &Path) -> PathBuf {
    let start = absolute(start);
    let mut cur: Option<&Path> = Some(&start);
    while let Some(dir) = cur {
        if dir.join(".git").exists() {
            return dir.to_path_buf();
        }
        cur = dir.parent();
    }
    start
}

/// Ruta legible: reemplaza el HOME por `~`.
pub fn pretty(p: &Path, home: &Path) -> String {
    match p.strip_prefix(home) {
        Ok(rest) if !home.as_os_str().is_empty() && home != Path::new("/") => {
            if rest.as_os_str().is_empty() {
                "~".into()
            } else {
                format!("~/{}", rest.display())
            }
        }
        _ => p.display().to_string(),
    }
}
