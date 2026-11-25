//! Importar/exportar .zip: ida y vuelta, respaldos y protecciones.

use skillbank_core::archive::{ExportOptions, ImportOptions, export_backup, export_skill, find_skill_dirs, import};
use skillbank_core::hash::tree_hash;
use skillbank_core::install::{PushOptions, push};
use skillbank_core::manage::{TagOptions, tag};
use skillbank_core::paths::Paths;
use skillbank_core::{Context, ErrorKind};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;

fn ctx(root: &Path, bank: &str) -> Context {
    let home = root.join("home");
    fs::create_dir_all(&home).unwrap();
    Context::load(Paths {
        home,
        config_file: root.join("config.toml"),
        data_dir: root.join("data"),
        state_dir: root.join(format!("state-{bank}")),
        bank_override: Some(root.join(bank)),
    })
    .unwrap()
}

fn skill(dir: &Path, name: &str, extra: &str) -> PathBuf {
    let s = dir.join(name);
    fs::create_dir_all(s.join("scripts")).unwrap();
    fs::write(
        s.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: Prueba. Úsala cuando haga falta.\n{extra}---\n# {name}\nPasos.\n"),
    )
    .unwrap();
    let sc = s.join("scripts/run.sh");
    fs::write(&sc, "#!/bin/sh\necho hi\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&sc, fs::Permissions::from_mode(0o755)).unwrap();
    }
    s
}

fn push_dir(c: &Context, dir: &Path) {
    push(c, &dir.display().to_string(), &PushOptions { cwd: dir.to_path_buf(), ..Default::default() }).unwrap();
}

fn make_zip(path: &Path, entries: &[(&str, &[u8])]) {
    let f = fs::File::create(path).unwrap();
    let mut w = zip::ZipWriter::new(f);
    for (name, data) in entries {
        w.start_file(*name, SimpleFileOptions::default()).unwrap();
        w.write_all(data).unwrap();
    }
    w.finish().unwrap();
}

#[test]
fn exportar_e_importar_ida_y_vuelta() {
    let t = tempfile::tempdir().unwrap();
    let a = ctx(t.path(), "bank-a");
    let src = skill(&t.path().join("src"), "demo", "");
    push_dir(&a, &src);
    let zip = t.path().join("demo.zip");
    let r = export_skill(&a, "demo", &zip, &ExportOptions::default()).unwrap();
    assert_eq!(r.files, 2);

    let b = ctx(t.path(), "bank-b");
    let out = import(&b, &zip, &ImportOptions::default()).unwrap();
    assert_eq!(out.kind, "zip");
    assert_eq!(out.imported, 1);
    assert_eq!(tree_hash(&b.bank.skill_dir("demo")).unwrap(), tree_hash(&src).unwrap());
    assert_eq!(b.bank.meta("demo").executables, vec!["scripts/run.sh"]);
    // Importar de nuevo: igual, no hace nada.
    let out = import(&b, &zip, &ImportOptions::default()).unwrap();
    assert_eq!(out.rows[0].status, "same");
    assert_eq!(out.imported, 0);
    // No quedan temporales.
    assert_eq!(fs::read_dir(b.bank.staging_dir()).unwrap().count(), 0);
}

#[test]
fn exportar_para_claude_ai() {
    let t = tempfile::tempdir().unwrap();
    let c = ctx(t.path(), "bank");
    push_dir(&c, &skill(&t.path().join("src"), "cc", "model: sonnet\nhooks:\n  Stop:\n    - x\n"));
    let zip = t.path().join("cc.zip");
    let opts = ExportOptions { claude_ai: true, strip_nonstandard: false };
    let e = export_skill(&c, "cc", &zip, &opts).unwrap_err();
    assert_eq!(e.kind, ErrorKind::Invalid);
    assert!(!zip.exists());

    let r = export_skill(&c, "cc", &zip, &ExportOptions { claude_ai: true, strip_nonstandard: true }).unwrap();
    assert_eq!(r.stripped_keys, vec!["hooks", "model"]);
    let mut ar = zip::ZipArchive::new(fs::File::open(&zip).unwrap()).unwrap();
    let mut md = String::new();
    std::io::Read::read_to_string(&mut ar.by_name("cc/SKILL.md").unwrap(), &mut md).unwrap();
    assert!(!md.contains("model:") && !md.contains("hooks:"), "{md}");
    // El banco no cambió.
    assert!(fs::read_to_string(c.bank.skill_dir("cc").join("SKILL.md")).unwrap().contains("model: sonnet"));
}

#[test]
fn respaldo_completo_conserva_etiquetas() {
    let t = tempfile::tempdir().unwrap();
    let a = ctx(t.path(), "bank-a");
    push_dir(&a, &skill(&t.path().join("src"), "uno", ""));
    push_dir(&a, &skill(&t.path().join("src"), "dos", ""));
    tag(&a, "uno", &TagOptions { add: vec!["pdf".into()], ..Default::default() }).unwrap();
    let zip = t.path().join("respaldo.zip");
    assert_eq!(export_backup(&a, &zip).unwrap().skills, 2);

    let b = ctx(t.path(), "bank-b");
    let out = import(&b, &zip, &ImportOptions::default()).unwrap();
    assert_eq!(out.kind, "backup");
    assert_eq!(out.imported, 2);
    assert_eq!(b.bank.meta("uno").tags, vec!["pdf"]);
}

#[test]
fn conflictos_al_importar() {
    let t = tempfile::tempdir().unwrap();
    let c = ctx(t.path(), "bank");
    push_dir(&c, &skill(&t.path().join("v1"), "demo", ""));
    let v2 = skill(&t.path().join("v2"), "demo", "license: MIT\n");
    skill(&t.path().join("v2"), "nueva", "");
    let src = t.path().join("v2");

    let e = import(&c, &src, &ImportOptions::default()).unwrap_err();
    assert_eq!(e.kind, ErrorKind::Conflict);
    assert!(!c.bank.has("nueva"), "en conflicto no se importa nada");

    let out = import(&c, &src, &ImportOptions { skip_existing: true, ..Default::default() }).unwrap();
    assert_eq!((out.imported, out.updated), (1, 0));
    assert!(!fs::read_to_string(c.bank.skill_dir("demo").join("SKILL.md")).unwrap().contains("MIT"));

    let out = import(&c, &src, &ImportOptions { force: true, ..Default::default() }).unwrap();
    assert_eq!(out.updated, 1);
    assert_eq!(tree_hash(&c.bank.skill_dir("demo")).unwrap(), tree_hash(&v2).unwrap());
}

#[test]
fn zip_con_ruta_peligrosa() {
    let t = tempfile::tempdir().unwrap();
    let c = ctx(t.path(), "bank");
    let zip = t.path().join("malo.zip");
    make_zip(&zip, &[("x/SKILL.md", b"---\nname: x\ndescription: d\n---\n"), ("../../fuera.txt", b"pwn")]);
    let e = import(&c, &zip, &ImportOptions::default()).unwrap_err();
    assert_eq!(e.kind, ErrorKind::Invalid);
    assert!(!t.path().join("fuera.txt").exists());
    assert!(!c.bank.has("x"));
}

#[test]
fn zip_con_symlink() {
    let t = tempfile::tempdir().unwrap();
    let c = ctx(t.path(), "bank");
    let zip = t.path().join("link.zip");
    let f = fs::File::create(&zip).unwrap();
    let mut w = zip::ZipWriter::new(f);
    w.start_file("x/SKILL.md", SimpleFileOptions::default()).unwrap();
    w.write_all(b"---\nname: x\ndescription: d\n---\n").unwrap();
    w.add_symlink("x/secreto", "/etc/passwd", SimpleFileOptions::default()).unwrap();
    w.finish().unwrap();
    let e = import(&c, &zip, &ImportOptions::default()).unwrap_err();
    assert_eq!(e.kind, ErrorKind::Invalid);
    assert!(e.message.contains("enlace simbólico"));
}

#[test]
fn zip_con_demasiadas_entradas() {
    let t = tempfile::tempdir().unwrap();
    let c = ctx(t.path(), "bank");
    let zip = t.path().join("bomba.zip");
    let names: Vec<String> = (0..2001).map(|i| format!("x/f{i}.txt")).collect();
    let entries: Vec<(&str, &[u8])> = names.iter().map(|n| (n.as_str(), b"x" as &[u8])).collect();
    make_zip(&zip, &entries);
    assert_eq!(import(&c, &zip, &ImportOptions::default()).unwrap_err().kind, ErrorKind::Invalid);
}

#[test]
fn zip_sin_carpeta_y_con_basura_de_mac() {
    let t = tempfile::tempdir().unwrap();
    let c = ctx(t.path(), "bank");
    let zip = t.path().join("suelto.zip");
    make_zip(
        &zip,
        &[
            ("SKILL.md", b"---\nname: suelta\ndescription: Prueba. Usala cuando quieras.\n---\nhola\n"),
            ("__MACOSX/._SKILL.md", b"basura"),
            ("assets/plantilla/SKILL.md", b"---\nname: interna\ndescription: no\n---\n"),
        ],
    );
    let out = import(&c, &zip, &ImportOptions::default()).unwrap();
    assert_eq!(out.rows.len(), 1, "solo la skill más externa: {:?}", out.rows);
    assert!(c.bank.has("suelta"));
    assert!(c.bank.skill_dir("suelta").join("assets/plantilla/SKILL.md").is_file());
    assert!(!c.bank.has("interna"));
}

#[test]
fn skills_invalidas_se_omiten() {
    let t = tempfile::tempdir().unwrap();
    let c = ctx(t.path(), "bank");
    let src = t.path().join("src");
    skill(&src, "buena", "");
    fs::create_dir_all(src.join("Mala")).unwrap();
    fs::write(src.join("Mala/SKILL.md"), "---\nname: Mala\ndescription: x\n---\n").unwrap();
    let out = import(&c, &src, &ImportOptions::default()).unwrap();
    let st: Vec<_> = out.rows.iter().map(|r| r.status.as_str()).collect();
    assert_eq!(st, vec!["invalid", "new"]);
    assert_eq!(out.imported, 1);
    assert_eq!(find_skill_dirs(&src).len(), 2);
}
