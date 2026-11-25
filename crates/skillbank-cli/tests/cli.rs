//! Pruebas de integración del binario `skillbank` en un HOME temporal.

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

struct Env {
    t: tempfile::TempDir,
}

struct Out {
    code: i32,
    json: Value,
    stdout: String,
    stderr: String,
}

impl Env {
    fn new() -> Env {
        let t = tempfile::tempdir().unwrap();
        fs::create_dir_all(t.path().join("home")).unwrap();
        Env { t }
    }

    fn home(&self) -> PathBuf {
        self.t.path().join("home")
    }

    fn bank(&self) -> PathBuf {
        self.home().join(".local/share/skillbank/bank")
    }

    /// Crea un proyecto (con `.git`) y devuelve su ruta.
    fn project(&self, name: &str) -> PathBuf {
        let p = self.t.path().join(name);
        fs::create_dir_all(p.join(".git")).unwrap();
        p
    }

    fn cmd(&self, cwd: &Path) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_skillbank"));
        c.current_dir(cwd).env("HOME", self.home());
        for v in [
            "XDG_CONFIG_HOME",
            "XDG_DATA_HOME",
            "XDG_STATE_HOME",
            "CLAUDECODE",
            "SKILLBANK_BANK",
            "SKILLBANK_CONFIG",
            "SKILLBANK_STATE_DIR",
        ] {
            c.env_remove(v);
        }
        c
    }

    fn run_with(&self, mut c: Command, args: &[&str]) -> Out {
        let o = c.args(args).output().unwrap();
        let stdout = String::from_utf8_lossy(&o.stdout).into_owned();
        Out {
            code: o.status.code().unwrap_or(-1),
            json: serde_json::from_str(&stdout).unwrap_or(Value::Null),
            stdout,
            stderr: String::from_utf8_lossy(&o.stderr).into_owned(),
        }
    }

    /// Ejecuta con `--json`.
    fn json(&self, cwd: &Path, args: &[&str]) -> Out {
        let mut a = args.to_vec();
        a.push("--json");
        self.run_with(self.cmd(cwd), &a)
    }
}

fn write_skill(dir: &Path, name: &str, extra_body: &str) -> PathBuf {
    let root = dir.join(name);
    fs::create_dir_all(root.join("scripts")).unwrap();
    fs::write(
        root.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: Skill de prueba. Úsala cuando haga falta probar.\n---\n# {name}\n\nInstrucciones.\n{extra_body}"),
    )
    .unwrap();
    let script = root.join("scripts/run.sh");
    fs::write(&script, "#!/bin/sh\necho hola\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    }
    root
}

fn append(path: &Path, text: &str) {
    let mut s = fs::read_to_string(path).unwrap();
    s.push_str(text);
    fs::write(path, s).unwrap();
}

#[test]
fn sobre_json_estable() {
    let e = Env::new();
    let a = e.project("a");
    write_skill(&a.join(".claude/skills"), "demo", "");
    let o = e.json(&a, &["push", "demo"]);
    assert_eq!(o.code, 0, "{}", o.stdout);
    let keys: Vec<&String> = o.json.as_object().unwrap().keys().collect();
    assert_eq!(keys, ["schema", "ok", "command", "code", "exit_code", "message", "data", "warnings"]);
    assert_eq!(o.json["schema"], 1);
    assert_eq!(o.json["ok"], true);
    assert_eq!(o.json["data"]["status"], "created");
    assert_eq!(o.json["data"]["registered"], true);
}

#[test]
fn push_pull_ida_y_vuelta() {
    let e = Env::new();
    let a = e.project("a");
    let src = write_skill(&a.join(".claude/skills"), "demo", "");
    assert_eq!(e.json(&a, &["push", ".claude/skills/demo", "--tags", "Pruebas, demo"]).code, 0);
    assert!(e.bank().join("skills/demo/SKILL.md").is_file());
    let meta: Value = serde_json::from_str(&fs::read_to_string(e.bank().join("meta/demo.json")).unwrap()).unwrap();
    assert_eq!(meta["tags"], serde_json::json!(["demo", "pruebas"]));
    assert_eq!(meta["executables"], serde_json::json!(["scripts/run.sh"]));

    let b = e.project("b");
    let o = e.json(&b, &["pull", "demo", "--agent", "claude"]);
    assert_eq!(o.code, 0, "{}", o.stdout);
    assert_eq!(o.json["data"]["items"][0]["action"], "installed");
    let dst = b.join(".claude/skills/demo");
    assert_eq!(fs::read(dst.join("SKILL.md")).unwrap(), fs::read(src.join("SKILL.md")).unwrap());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(dst.join("scripts/run.sh")).unwrap().permissions().mode();
        assert!(mode & 0o111 != 0, "el script debe seguir siendo ejecutable");
    }
    // Repetir: sin cambios.
    let o = e.json(&b, &["pull", "demo", "--agent", "claude"]);
    assert_eq!(o.json["data"]["items"][0]["action"], "unchanged");
}

#[test]
fn push_sin_cambios_conflicto_y_force() {
    let e = Env::new();
    let a = e.project("a");
    let src = write_skill(&a.join(".claude/skills"), "demo", "");
    assert_eq!(e.json(&a, &["push", "demo"]).code, 0);
    let o = e.json(&a, &["push", "demo"]);
    assert_eq!(o.json["data"]["status"], "unchanged");

    append(&src.join("SKILL.md"), "Paso nuevo.\n");
    let o = e.json(&a, &["push", "demo"]);
    assert_eq!(o.code, 5);
    assert_eq!(o.json["code"], "CONFLICT");
    assert_eq!(o.json["data"]["relation"], "fast_forward");
    assert_eq!(o.json["data"]["diff"]["files"][0]["path"], "SKILL.md");
    assert_eq!(o.json["data"]["diff"]["files"][0]["added"], 1);
    assert!(o.json["hint"].as_str().unwrap().contains("--force"));
    // El banco no cambió.
    assert!(!fs::read_to_string(e.bank().join("skills/demo/SKILL.md")).unwrap().contains("Paso nuevo"));

    let o = e.json(&a, &["push", "demo", "--force"]);
    assert_eq!(o.code, 0);
    assert_eq!(o.json["data"]["status"], "updated");
    assert!(fs::read_to_string(e.bank().join("skills/demo/SKILL.md")).unwrap().contains("Paso nuevo"));
    // La versión anterior quedó en la papelera.
    assert!(fs::read_dir(e.bank().join(".skillbank/trash")).unwrap().count() >= 1);
}

#[test]
fn pull_actualiza_sola_una_copia_sin_tocar() {
    let e = Env::new();
    let a = e.project("a");
    let src = write_skill(&a.join(".claude/skills"), "demo", "");
    e.json(&a, &["push", "demo"]);
    let b = e.project("b");
    e.json(&b, &["pull", "demo", "--agent", "claude"]);

    append(&src.join("SKILL.md"), "Mejora.\n");
    assert_eq!(e.json(&a, &["push", "demo", "--force"]).code, 0);
    let o = e.json(&b, &["pull", "demo", "--agent", "claude"]);
    assert_eq!(o.code, 0, "{}", o.stdout);
    assert_eq!(o.json["data"]["items"][0]["action"], "updated");
    assert!(fs::read_to_string(b.join(".claude/skills/demo/SKILL.md")).unwrap().contains("Mejora"));
}

#[test]
fn pull_con_cambios_locales_es_conflicto() {
    let e = Env::new();
    let a = e.project("a");
    write_skill(&a.join(".claude/skills"), "demo", "");
    e.json(&a, &["push", "demo"]);
    let b = e.project("b");
    e.json(&b, &["pull", "demo", "--agent", "claude"]);
    let local = b.join(".claude/skills/demo/SKILL.md");
    append(&local, "Cambio local valioso.\n");

    let o = e.json(&b, &["pull", "demo", "--agent", "claude"]);
    assert_eq!(o.code, 5);
    assert_eq!(o.json["data"]["items"][0]["reason"], "modified");
    assert!(fs::read_to_string(&local).unwrap().contains("Cambio local valioso"));

    let o = e.json(&b, &["pull", "demo", "--agent", "claude", "--force"]);
    assert_eq!(o.code, 0);
    assert_eq!(o.json["data"]["items"][0]["reason"], "forced");
    assert!(!fs::read_to_string(&local).unwrap().contains("Cambio local valioso"));
}

#[test]
fn pull_sobre_copia_ajena_es_conflicto() {
    let e = Env::new();
    let a = e.project("a");
    write_skill(&a.join(".claude/skills"), "demo", "");
    e.json(&a, &["push", "demo"]);
    let c = e.project("c");
    write_skill(&c.join(".claude/skills"), "demo", "Otra versión.\n");
    let o = e.json(&c, &["pull", "demo", "--agent", "claude"]);
    assert_eq!(o.code, 5);
    assert_eq!(o.json["data"]["items"][0]["reason"], "unregistered");
}

#[cfg(unix)]
#[test]
fn pull_sobre_symlink_no_toca_el_destino_del_enlace() {
    let e = Env::new();
    let a = e.project("a");
    write_skill(&a.join(".claude/skills"), "demo", "");
    e.json(&a, &["push", "demo"]);
    let real = write_skill(&e.t.path().join("otro-lugar"), "demo", "Original enlazado.\n");
    let d = e.project("d");
    fs::create_dir_all(d.join(".claude/skills")).unwrap();
    std::os::unix::fs::symlink(&real, d.join(".claude/skills/demo")).unwrap();

    let o = e.json(&d, &["pull", "demo", "--agent", "claude"]);
    assert_eq!(o.code, 5);
    assert_eq!(o.json["data"]["items"][0]["reason"], "symlink");

    assert_eq!(e.json(&d, &["pull", "demo", "--agent", "claude", "--force"]).code, 0);
    assert!(!fs::symlink_metadata(d.join(".claude/skills/demo")).unwrap().file_type().is_symlink());
    assert!(fs::read_to_string(real.join("SKILL.md")).unwrap().contains("Original enlazado"));
}

#[test]
fn skill_invalida_no_se_sube() {
    let e = Env::new();
    let a = e.project("a");
    let dir = a.join(".claude/skills/Mala_Skill");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("SKILL.md"), "---\nname: Mala_Skill\ndescription: x\n---\nhola\n").unwrap();
    let o = e.json(&a, &["push", ".claude/skills/Mala_Skill"]);
    assert_eq!(o.code, 4);
    assert_eq!(o.json["code"], "INVALID");
    assert!(o.json["data"]["diagnostics"].as_array().unwrap().iter().any(|d| d["code"] == "E_NAME_CHARS"));
    // --allow-invalid no salta los errores de name.
    assert_eq!(e.json(&a, &["push", ".claude/skills/Mala_Skill", "--allow-invalid"]).code, 4);
    assert!(!e.bank().join("skills").exists() || fs::read_dir(e.bank().join("skills")).unwrap().count() == 0);
}

#[test]
fn no_encontrada_sugiere_nombres() {
    let e = Env::new();
    let a = e.project("a");
    write_skill(&a.join(".claude/skills"), "pdf-tools", "");
    e.json(&a, &["push", "pdf-tools"]);
    let o = e.json(&a, &["pull", "pdf-tool"]);
    assert_eq!(o.code, 3);
    assert_eq!(o.json["data"]["did_you_mean"][0], "pdf-tools");
}

#[test]
fn nombre_ambiguo() {
    let e = Env::new();
    let a = e.project("a");
    write_skill(&a.join(".claude/skills"), "x", "");
    write_skill(&a.join(".agents/skills"), "x", "Distinta.\n");
    let o = e.json(&a, &["push", "x"]);
    assert_eq!(o.code, 9, "{}", o.stdout);
    assert_eq!(o.json["data"]["candidates"].as_array().unwrap().len(), 2);
}

#[test]
fn banco_ocupado() {
    let e = Env::new();
    let a = e.project("a");
    write_skill(&a.join(".claude/skills"), "demo", "");
    e.json(&a, &["push", "demo"]);
    let f = fs::OpenOptions::new().read(true).write(true).open(e.bank().join(".skillbank/bank.lock")).unwrap();
    f.lock().unwrap();
    let o = e.json(&a, &["remove", "demo", "--yes", "--lock-timeout", "0"]);
    assert_eq!(o.code, 6);
    assert_eq!(o.json["code"], "LOCKED");
    f.unlock().unwrap();
    assert_eq!(e.json(&a, &["remove", "demo", "--yes"]).code, 0);
}

#[cfg(unix)]
#[test]
fn registro_sin_permisos_es_solo_un_aviso() {
    use std::os::unix::fs::PermissionsExt;
    let e = Env::new();
    let a = e.project("a");
    write_skill(&a.join(".claude/skills"), "demo", "");
    let ro = e.t.path().join("solo-lectura");
    fs::create_dir_all(&ro).unwrap();
    fs::set_permissions(&ro, fs::Permissions::from_mode(0o500)).unwrap();
    let mut c = e.cmd(&a);
    c.env("SKILLBANK_STATE_DIR", ro.join("state"));
    let o = e.run_with(c, &["push", "demo", "--json"]);
    fs::set_permissions(&ro, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(o.code, 0, "{}", o.stdout);
    assert!(o.json["warnings"].as_array().unwrap().iter().any(|w| w["code"] == "REGISTRY_NOT_WRITTEN"));
}

#[test]
fn errores_de_uso_en_json() {
    let e = Env::new();
    let a = e.project("a");
    let o = e.json(&a, &["pull"]);
    assert_eq!(o.code, 2);
    assert_eq!(o.json["code"], "USAGE");
    let o = e.json(&a, &["remove", "nada"]);
    assert_eq!(o.code, 3);
    write_skill(&a.join(".claude/skills"), "demo", "");
    e.json(&a, &["push", "demo"]);
    let o = e.json(&a, &["remove", "demo"]);
    assert_eq!(o.code, 2, "remove sin --yes debe fallar");
    assert!(e.bank().join("skills/demo").exists());
}

#[test]
fn dedupe_y_destinos() {
    let e = Env::new();
    let a = e.project("a");
    write_skill(&a.join(".claude/skills"), "demo", "");
    e.json(&a, &["push", "demo"]);
    let b = e.project("b");
    let o = e.json(&b, &["pull", "demo", "--agent", "claude", "--agent", "cursor"]);
    assert_eq!(o.json["data"]["items"].as_array().unwrap().len(), 1);
    assert!(b.join(".claude/skills/demo").is_dir());
    assert!(!b.join(".cursor/skills/demo").exists());

    let o = e.json(&b, &["pull", "demo", "--agent", "claude,codex"]);
    assert_eq!(o.json["data"]["items"].as_array().unwrap().len(), 2);
    assert!(b.join(".agents/skills/demo").is_dir());

    let o = e.json(&b, &["pull", "demo", "--agent", "codex", "--global"]);
    assert_eq!(o.code, 0);
    assert!(e.home().join(".agents/skills/demo/SKILL.md").is_file());
}

#[test]
fn deteccion_por_entorno_y_marcadores() {
    let e = Env::new();
    let a = e.project("a");
    write_skill(&a.join(".claude/skills"), "demo", "");
    e.json(&a, &["push", "demo"]);
    let b = e.project("b");
    fs::create_dir_all(b.join(".cursor")).unwrap();
    // Sin --agent ni entorno: marcador .cursor → Cursor.
    let o = e.json(&b, &["pull", "demo", "--dry-run"]);
    assert_eq!(o.json["data"]["agent_source"], "project_markers");
    assert_eq!(o.json["data"]["agents"], serde_json::json!(["cursor"]));
    // Con CLAUDECODE=1: Claude Code.
    let mut c = e.cmd(&b);
    c.env("CLAUDECODE", "1");
    let o = e.run_with(c, &["pull", "demo", "--json"]);
    assert_eq!(o.json["data"]["agent_source"], "environment");
    assert!(b.join(".claude/skills/demo").is_dir());
}

#[test]
fn list_search_show_validate() {
    let e = Env::new();
    let a = e.project("a");
    write_skill(&a.join(".claude/skills"), "pdf-tools", "Extrae tablas de facturas.\n");
    let cc = write_skill(&a.join(".claude/skills"), "solo-cc", "");
    let md = cc.join("SKILL.md");
    let text = fs::read_to_string(&md).unwrap().replacen("---\n#", "model: sonnet\n---\n#", 1);
    fs::write(&md, text).unwrap();
    e.json(&a, &["push", "pdf-tools", "--tags", "pdf"]);
    e.json(&a, &["push", "solo-cc"]);

    let o = e.json(&a, &["list"]);
    assert_eq!(o.json["data"].as_array().unwrap().len(), 2);
    let o = e.json(&a, &["list", "--tag", "pdf"]);
    assert_eq!(o.json["data"].as_array().unwrap().len(), 1);

    let o = e.json(&a, &["search", "facturas"]);
    assert_eq!(o.json["data"][0]["name"], "pdf-tools");
    assert_eq!(o.json["data"][0]["matched_in"], serde_json::json!(["body"]));

    let o = e.json(&a, &["show", "pdf-tools", "--body"]);
    assert!(o.json["data"]["body"].as_str().unwrap().contains("facturas"));
    assert_eq!(o.json["data"]["installs"].as_array().unwrap().len(), 1);
    assert_eq!(o.json["data"]["installs"][0]["status"], "up_to_date");

    assert_eq!(e.json(&a, &["validate", "solo-cc"]).code, 0);
    let o = e.json(&a, &["validate", "solo-cc", "--profile", "claude-ai"]);
    assert_eq!(o.code, 4);
    assert!(o.json["data"]["diagnostics"].as_array().unwrap().iter().any(|d| d["code"] == "C_NONSTANDARD_KEY"));

    let o = e.json(&a, &["path", "pdf-tools"]);
    assert!(o.json["message"].as_str().unwrap().ends_with("skills/pdf-tools"));
}

#[test]
fn guia_para_agentes() {
    let e = Env::new();
    fs::create_dir_all(e.home().join(".claude")).unwrap();
    let a = e.project("a");
    let o = e.json(&a, &["agents", "install-guide"]);
    assert_eq!(o.code, 0, "{}", o.stdout);
    let guide = e.home().join(".claude/skills/banco-de-skills/SKILL.md");
    assert!(guide.is_file());
    assert!(e.home().join(".claude/skills/banco-de-skills/references/comandos.md").is_file());
    assert!(!fs::read_to_string(&guide).unwrap().contains("{{"));
    let o = e.json(&a, &["agents", "install-guide"]);
    assert_eq!(o.json["data"][0]["action"], "unchanged");

    // Una skill ajena con el mismo nombre no se pisa sin --force.
    fs::create_dir_all(e.home().join(".codex")).unwrap();
    let foreign = e.home().join(".agents/skills/banco-de-skills");
    fs::create_dir_all(&foreign).unwrap();
    fs::write(foreign.join("SKILL.md"), "---\nname: banco-de-skills\ndescription: otra\n---\nx\n").unwrap();
    let o = e.json(&a, &["agents", "install-guide", "--agent", "codex"]);
    assert_eq!(o.code, 5);

    let o = e.json(&a, &["agents", "install-guide", "--uninstall", "--agent", "claude"]);
    assert_eq!(o.code, 0);
    assert!(!guide.exists());
}

#[test]
fn completions_y_ayuda() {
    let e = Env::new();
    let a = e.project("a");
    let o = e.run_with(e.cmd(&a), &["completions", "bash"]);
    assert_eq!(o.code, 0);
    assert!(o.stdout.contains("skillbank"));
    let o = e.run_with(e.cmd(&a), &["--help"]);
    assert_eq!(o.code, 0);
    assert!(o.stdout.contains("Banco de Skills"));
    let o = e.run_with(e.cmd(&a), &["list"]);
    assert_eq!(o.code, 0);
    assert!(o.stdout.contains("vacío"), "{}{}", o.stdout, o.stderr);
}

// ───────────── Fase 1b ─────────────

#[test]
fn outdated_e_installed() {
    let e = Env::new();
    let a = e.project("a");
    let src = write_skill(&a.join(".claude/skills"), "demo", "");
    e.json(&a, &["push", "demo"]);
    let b = e.project("b");
    e.json(&b, &["pull", "demo", "--agent", "claude"]);
    let c = e.project("c");
    e.json(&c, &["pull", "demo", "--agent", "claude"]);
    append(&c.join(".claude/skills/demo/SKILL.md"), "cambio en c\n");

    let o = e.json(&a, &["installed", "demo"]);
    assert_eq!(o.json["data"].as_array().unwrap().len(), 3);

    append(&src.join("SKILL.md"), "mejora\n");
    e.json(&a, &["push", "demo", "--force"]);
    let o = e.json(&b, &["outdated"]);
    let statuses: Vec<String> =
        o.json["data"]["items"].as_array().unwrap().iter().map(|i| i["status"].as_str().unwrap().to_string()).collect();
    assert!(statuses.contains(&"outdated".to_string()) && statuses.contains(&"diverged".to_string()), "{statuses:?}");

    let o = e.json(&b, &["outdated", "--project", ".", "--update"]);
    assert_eq!(o.json["data"]["updated"].as_array().unwrap().len(), 1);
    assert!(fs::read_to_string(b.join(".claude/skills/demo/SKILL.md")).unwrap().contains("mejora"));
    // La copia con cambios locales (c) no se tocó.
    let o = e.json(&c, &["outdated", "--project", ".", "--update"]);
    assert_eq!(o.json["data"]["updated"].as_array().unwrap().len(), 0);
    assert!(fs::read_to_string(c.join(".claude/skills/demo/SKILL.md")).unwrap().contains("cambio en c"));
}

#[cfg(unix)]
#[test]
fn scan_global_con_symlinks_y_synced() {
    let e = Env::new();
    let h = e.home();
    let real = write_skill(&h.join(".agents/skills"), "comun", "");
    fs::create_dir_all(h.join(".claude/skills/synced")).unwrap();
    std::os::unix::fs::symlink(&real, h.join(".claude/skills/comun")).unwrap();
    write_skill(&h.join(".claude/skills/synced"), "de-claude-ai", "");
    write_skill(&h.join(".claude/skills"), "propia", "");
    let a = e.project("a");

    let o = e.json(&a, &["scan", "--global"]);
    let entries = o.json["data"]["entries"].as_array().unwrap().clone();
    let names: Vec<&str> = entries.iter().map(|x| x["name"].as_str().unwrap()).collect();
    assert_eq!(names.iter().filter(|n| **n == "comun").count(), 1, "el symlink no se cuenta dos veces: {names:?}");
    let synced = entries.iter().find(|x| x["name"] == "de-claude-ai").unwrap();
    assert_eq!(synced["synced"], true);
    assert!(entries.iter().all(|x| x["status"] == "absent"));

    let o = e.json(&a, &["scan", "--global", "--import"]);
    assert_eq!(o.json["data"]["imported"], 3);
    let o = e.json(&a, &["list"]);
    assert_eq!(o.json["data"].as_array().unwrap().len(), 3);
    // Solo se registran copias reales que no vienen de claude.ai.
    let o = e.json(&a, &["installed"]);
    let paths: Vec<String> =
        o.json["data"].as_array().unwrap().iter().map(|x| x["path"].as_str().unwrap().to_string()).collect();
    assert!(paths.iter().any(|p| p.ends_with(".agents/skills/comun")), "{paths:?}");
    assert!(paths.iter().any(|p| p.ends_with(".claude/skills/propia")), "{paths:?}");
    assert!(!paths.iter().any(|p| p.contains("synced")), "{paths:?}");
}

#[test]
fn tag_rename_new() {
    let e = Env::new();
    let a = e.project("a");
    let o = e.json(
        &a,
        &["new", "mi-skill", "--template", "con-scripts", "--dir", ".claude/skills", "-d", "Hace X. Úsala cuando Y."],
    );
    assert_eq!(o.code, 0, "{}", o.stdout);
    assert!(a.join(".claude/skills/mi-skill/scripts/ejemplo.sh").is_file());
    assert_eq!(e.json(&a, &["new", "Mala Skill"]).code, 4);
    assert_eq!(e.json(&a, &["new", "x", "--template", "nope"]).code, 2);
    assert_eq!(e.json(&a, &["validate", ".claude/skills/mi-skill", "--profile", "claude-ai", "--strict"]).code, 0);

    let o = e.json(&a, &["new", "en-banco", "--in-bank", "--tags", "uno,dos"]);
    assert_eq!(o.code, 0, "{}", o.stdout);
    assert_eq!(e.json(&a, &["new", "en-banco", "--in-bank"]).code, 5);

    let o = e.json(&a, &["tag", "en-banco", "--add", "tres", "--remove", "uno"]);
    assert_eq!(o.json["data"]["tags"], serde_json::json!(["dos", "tres"]));
    let o = e.json(&a, &["tag"]);
    assert_eq!(o.json["data"].as_array().unwrap().len(), 2);

    let o = e.json(&a, &["rename", "en-banco", "renombrada"]);
    assert_eq!(o.code, 0, "{}", o.stdout);
    assert!(fs::read_to_string(e.bank().join("skills/renombrada/SKILL.md")).unwrap().contains("name: renombrada"));
    assert!(e.bank().join("meta/renombrada.json").is_file());
    assert!(!e.bank().join("meta/en-banco.json").exists());
    assert_eq!(e.json(&a, &["validate", "renombrada"]).code, 0);
    assert_eq!(e.json(&a, &["rename", "renombrada", "Mal"]).code, 4);
}

#[test]
fn exportar_e_importar_desde_el_cli() {
    let e = Env::new();
    let a = e.project("a");
    write_skill(&a.join(".claude/skills"), "demo", "");
    e.json(&a, &["push", "demo", "--tags", "x"]);
    let out = e.t.path().join("out");
    let o = e.json(&a, &["export", "demo", "-o", out.to_str().unwrap()]);
    assert_eq!(o.code, 0, "{}", o.stdout);
    assert!(out.join("demo.zip").is_file());
    let o = e.json(&a, &["export", "--all", "-o", out.join("respaldo.zip").to_str().unwrap()]);
    assert_eq!(o.code, 0, "{}", o.stdout);

    // Banco nuevo (otro HOME) a partir del respaldo.
    let e2 = Env::new();
    let b = e2.project("b");
    let o = e2.json(&b, &["import", out.join("respaldo.zip").to_str().unwrap(), "--dry-run"]);
    assert_eq!(o.json["data"]["imported"], 1);
    assert!(!e2.bank().join("skills/demo").exists());
    let o = e2.json(&b, &["import", out.join("respaldo.zip").to_str().unwrap()]);
    assert_eq!(o.code, 0, "{}", o.stdout);
    let o = e2.json(&b, &["list", "--tag", "x"]);
    assert_eq!(o.json["data"].as_array().unwrap().len(), 1);

    // Conflicto al importar una versión distinta.
    let other = e2.t.path().join("otra");
    write_skill(&other, "demo", "Distinta.\n");
    let o = e2.json(&b, &["import", other.to_str().unwrap()]);
    assert_eq!(o.code, 5);
    assert_eq!(o.json["data"]["rows"][0]["action"], "conflict");
}

#[test]
fn doctor_y_fix() {
    let e = Env::new();
    fs::create_dir_all(e.home().join(".claude")).unwrap();
    let a = e.project("a");
    write_skill(&a.join(".claude/skills"), "demo", "");
    e.json(&a, &["push", "demo"]);
    let b = e.project("b");
    e.json(&b, &["pull", "demo", "--agent", "claude"]);
    fs::remove_dir_all(b.join(".claude/skills/demo")).unwrap();
    fs::remove_file(e.bank().join("meta/demo.json")).unwrap();

    let o = e.json(&a, &["doctor"]);
    assert_eq!(o.code, 0, "{}", o.stdout);
    let ids: Vec<(String, String)> = o.json["data"]["checks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| (c["id"].as_str().unwrap().to_string(), c["status"].as_str().unwrap().to_string()))
        .collect();
    for want in ["guide", "meta-missing", "registry"] {
        assert!(ids.iter().any(|(id, st)| id == want && st == "warn"), "{want}: {ids:?}");
    }

    let o = e.json(&a, &["doctor", "--fix"]);
    assert!(o.json["data"]["fixed"].as_u64().unwrap() >= 3, "{}", o.stdout);
    assert!(e.bank().join("meta/demo.json").is_file());
    assert!(e.home().join(".claude/skills/banco-de-skills/SKILL.md").is_file());
    let o = e.json(&a, &["installed"]);
    assert_eq!(o.json["data"].as_array().unwrap().len(), 1);
    let o = e.json(&a, &["doctor"]);
    let warns: Vec<&Value> = o.json["data"]["checks"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["status"] == "warn" && c["id"] != "cli")
        .collect();
    assert!(warns.is_empty(), "{warns:?}");
}

#[test]
fn scan_import_no_pisa_copias_distintas_con_el_mismo_nombre() {
    let e = Env::new();
    let h = e.home();
    write_skill(&h.join(".claude/skills"), "dup", "Versión de Claude.\n");
    write_skill(&h.join(".agents/skills"), "dup", "Versión de Codex.\n");
    let a = e.project("a");
    let o = e.json(&a, &["scan", "--global", "--import"]);
    assert_eq!(o.json["data"]["imported"], 1, "{}", o.stdout);
    let st: Vec<&str> =
        o.json["data"]["entries"].as_array().unwrap().iter().map(|x| x["status"].as_str().unwrap()).collect();
    assert_eq!(st, vec!["same", "different"]);
    assert!(fs::read_to_string(e.bank().join("skills/dup/SKILL.md")).unwrap().contains("Versión de Claude"));
}
