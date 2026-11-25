//! Invariantes del hash de contenido y de la copia de skills.

use skillbank_core::fsutil::{copy_tree, list_files, replace_dir_with_copy};
use skillbank_core::hash::tree_hash;
use std::fs;
use std::path::Path;

fn write(root: &Path, rel: &str, content: &str) {
    let p = root.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, content).unwrap();
}

fn sample(root: &Path) {
    write(root, "SKILL.md", "---\nname: s\ndescription: d\n---\nhola\n");
    write(root, "scripts/run.sh", "#!/bin/sh\necho hi\n");
    write(root, "references/a.md", "# A\n");
}

#[test]
fn la_copia_conserva_el_hash() {
    let t = tempfile::tempdir().unwrap();
    let src = t.path().join("src");
    sample(&src);
    let dst = t.path().join("dst");
    copy_tree(&src, &dst).unwrap();
    assert_eq!(tree_hash(&src).unwrap(), tree_hash(&dst).unwrap());
}

#[test]
fn ignora_basura_permisos_y_fechas() {
    let t = tempfile::tempdir().unwrap();
    let a = t.path().join("a");
    sample(&a);
    let h = tree_hash(&a).unwrap();
    write(&a, ".DS_Store", "x");
    write(&a, "__pycache__/m.pyc", "x");
    write(&a, ".git/HEAD", "x");
    write(&a, "notas.swp", "x");
    write(&a, "SKILL.md.sbconflict1", "x");
    fs::create_dir_all(a.join("vacia")).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(a.join("scripts/run.sh"), fs::Permissions::from_mode(0o755)).unwrap();
    }
    assert_eq!(tree_hash(&a).unwrap(), h);
    // Y la copia no se lleva la basura.
    let dst = t.path().join("dst");
    copy_tree(&a, &dst).unwrap();
    assert!(!dst.join(".git").exists());
    assert!(!dst.join(".DS_Store").exists());
}

#[test]
fn el_contenido_y_las_rutas_cambian_el_hash() {
    let t = tempfile::tempdir().unwrap();
    let a = t.path().join("a");
    sample(&a);
    let h = tree_hash(&a).unwrap();
    write(&a, "references/a.md", "# A cambiado\n");
    assert_ne!(tree_hash(&a).unwrap(), h);
    let h2 = tree_hash(&a).unwrap();
    fs::rename(a.join("references/a.md"), a.join("references/b.md")).unwrap();
    assert_ne!(tree_hash(&a).unwrap(), h2);
}

#[cfg(unix)]
#[test]
fn symlinks_internos_se_copian_como_contenido() {
    let t = tempfile::tempdir().unwrap();
    let a = t.path().join("a");
    sample(&a);
    write(t.path(), "fuera/extra.md", "extra\n");
    std::os::unix::fs::symlink(t.path().join("fuera/extra.md"), a.join("references/extra.md")).unwrap();
    let dst = t.path().join("dst");
    copy_tree(&a, &dst).unwrap();
    assert!(!fs::symlink_metadata(dst.join("references/extra.md")).unwrap().file_type().is_symlink());
    assert_eq!(tree_hash(&a).unwrap(), tree_hash(&dst).unwrap());
    assert_eq!(list_files(&dst).unwrap().len(), 4);
}

#[cfg(unix)]
#[test]
fn reemplazar_un_symlink_no_toca_su_destino() {
    let t = tempfile::tempdir().unwrap();
    let src = t.path().join("src");
    sample(&src);
    let real = t.path().join("real");
    write(&real, "SKILL.md", "original\n");
    let link = t.path().join("dest/skill");
    fs::create_dir_all(link.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&real, &link).unwrap();
    replace_dir_with_copy(&src, &link).unwrap();
    assert!(!fs::symlink_metadata(&link).unwrap().file_type().is_symlink());
    assert_eq!(fs::read_to_string(real.join("SKILL.md")).unwrap(), "original\n");
    assert_eq!(tree_hash(&link).unwrap(), tree_hash(&src).unwrap());
}

#[test]
fn reemplazar_un_directorio_existente() {
    let t = tempfile::tempdir().unwrap();
    let src = t.path().join("src");
    sample(&src);
    let dest = t.path().join("dest/skill");
    write(&dest, "viejo.md", "x");
    replace_dir_with_copy(&src, &dest).unwrap();
    assert!(!dest.join("viejo.md").exists());
    assert_eq!(tree_hash(&dest).unwrap(), tree_hash(&src).unwrap());
    // No quedan temporales en la carpeta padre.
    let leftovers: Vec<_> = fs::read_dir(dest.parent().unwrap()).unwrap().flatten().collect();
    assert_eq!(leftovers.len(), 1);
}
