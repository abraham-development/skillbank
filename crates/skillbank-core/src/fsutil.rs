//! Utilidades de sistema de archivos: lista de ignorados, recorrido de una skill,
//! copia de árboles, escritura atómica y bits de ejecución.

use crate::error::{CoreError, IoContext, Result};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Directorios que nunca forman parte de una skill (ni en hash, ni en copia, ni en zip).
const IGNORED_DIRS: &[&str] = &[".git", "__pycache__", "__MACOSX", ".skillbank"];
/// Archivos ignorados por nombre exacto.
const IGNORED_FILES: &[&str] = &[".DS_Store", "Thumbs.db"];

/// ¿Se ignora este componente de ruta (nombre de archivo o carpeta)?
pub fn is_ignored_name(name: &str, is_dir: bool) -> bool {
    if is_dir {
        return IGNORED_DIRS.contains(&name);
    }
    IGNORED_FILES.contains(&name)
        || name.ends_with(".pyc")
        || name.ends_with(".swp")
        || name.ends_with('~')
        || name.contains(".sbconflict")
        || name.starts_with(".skillbank-tmp")
}

/// Archivo dentro de una skill.
#[derive(Debug, Clone)]
pub struct SkillFile {
    /// Ruta relativa con separadores `/`.
    pub rel: String,
    pub abs: PathBuf,
    pub size: u64,
    pub executable: bool,
}

/// Recorre los archivos de una skill (sigue symlinks internos, ignora la lista negra),
/// ordenados por ruta relativa.
pub fn list_files(root: &Path) -> Result<Vec<SkillFile>> {
    let mut out = Vec::new();
    let walker = WalkDir::new(root).follow_links(true).into_iter().filter_entry(|e| {
        if e.depth() == 0 {
            return true;
        }
        let name = e.file_name().to_string_lossy();
        !is_ignored_name(&name, e.file_type().is_dir())
    });
    for entry in walker {
        let entry = entry.map_err(|e| {
            if e.loop_ancestor().is_some() {
                CoreError::other(format!("bucle de enlaces simbólicos en {}", root.display()))
            } else {
                CoreError::other(format!("no se pudo leer {}: {e}", root.display()))
            }
        })?;
        if !entry.file_type().is_file() {
            continue;
        }
        let rel_path = entry.path().strip_prefix(root).map_err(|e| CoreError::other(e.to_string()))?;
        let rel =
            rel_path.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/");
        let meta = entry.metadata().map_err(|e| CoreError::other(e.to_string()))?;
        out.push(SkillFile {
            rel,
            abs: entry.path().to_path_buf(),
            size: meta.len(),
            executable: is_executable(&meta),
        });
    }
    out.sort_by(|a, b| a.rel.as_bytes().cmp(b.rel.as_bytes()));
    Ok(out)
}

#[cfg(unix)]
fn is_executable(meta: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    meta.permissions().mode() & 0o111 != 0
}
#[cfg(not(unix))]
fn is_executable(_meta: &fs::Metadata) -> bool {
    false
}

/// Copia una skill de `src` a `dst` (que no debe existir). Respeta la lista de ignorados
/// y copia el contenido de los symlinks (nunca crea symlinks).
pub fn copy_tree(src: &Path, dst: &Path) -> Result<Vec<SkillFile>> {
    if dst.exists() {
        return Err(CoreError::other(format!("el destino ya existe: {}", dst.display())));
    }
    fs::create_dir_all(dst).at(dst)?;
    let files = list_files(src)?;
    for f in &files {
        let target = dst.join(&f.rel);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).at(parent)?;
        }
        fs::copy(&f.abs, &target).at(&f.abs)?;
    }
    Ok(files)
}

/// Marca como ejecutables los archivos indicados (rutas relativas).
pub fn set_executables(root: &Path, rels: &[String]) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for rel in rels {
            let p = root.join(rel);
            if let Ok(meta) = fs::metadata(&p) {
                let mut perm = meta.permissions();
                perm.set_mode(perm.mode() | 0o755);
                fs::set_permissions(&p, perm).at(&p)?;
            }
        }
    }
    let _ = (root, rels);
    Ok(())
}

/// Escribe un archivo de forma atómica (temporal + fsync + rename).
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(parent).at(parent)?;
    let tmp = parent.join(format!(
        ".{}.tmp-{}",
        path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
        uuid::Uuid::new_v4().simple()
    ));
    {
        let mut f = fs::File::create(&tmp).at(&tmp)?;
        f.write_all(bytes).at(&tmp)?;
        f.sync_all().at(&tmp)?;
    }
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        CoreError::other(format!("{}: {e}", path.display()))
    })
}

/// Reemplaza (o crea) el directorio `target` con el contenido de `src`, copiando primero a un
/// temporal hermano y cambiándolo de sitio con `rename`. Si `target` es un symlink, se reemplaza
/// el enlace en sí, nunca su destino.
pub fn replace_dir_with_copy(src: &Path, target: &Path) -> Result<Vec<SkillFile>> {
    let parent = target.parent().ok_or_else(|| CoreError::other("destino sin carpeta padre"))?;
    fs::create_dir_all(parent).at(parent)?;
    let name = target.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp_new = parent.join(format!(".skillbank-tmp-new-{name}-{}", uuid::Uuid::new_v4().simple()));
    let files = match copy_tree(src, &tmp_new) {
        Ok(f) => f,
        Err(e) => {
            let _ = fs::remove_dir_all(&tmp_new);
            return Err(e);
        }
    };
    let meta = fs::symlink_metadata(target);
    match meta {
        Ok(m) if m.file_type().is_symlink() || m.is_file() => {
            fs::remove_file(target).at(target)?;
            fs::rename(&tmp_new, target).at(target)?;
        }
        Ok(_) => {
            let tmp_old = parent.join(format!(".skillbank-tmp-old-{name}-{}", uuid::Uuid::new_v4().simple()));
            fs::rename(target, &tmp_old).at(target)?;
            if let Err(e) = fs::rename(&tmp_new, target) {
                let _ = fs::rename(&tmp_old, target);
                let _ = fs::remove_dir_all(&tmp_new);
                return Err(CoreError::other(format!("{}: {e}", target.display())));
            }
            let _ = fs::remove_dir_all(&tmp_old);
        }
        Err(_) => {
            fs::rename(&tmp_new, target).at(target)?;
        }
    }
    Ok(files)
}

/// ¿Es `p` (sin seguir enlaces) un symlink?
pub fn is_symlink(p: &Path) -> bool {
    fs::symlink_metadata(p).map(|m| m.file_type().is_symlink()).unwrap_or(false)
}
