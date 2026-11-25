//! Tabla de casos del validador (una o más filas por regla de docs/spec-validator.md).

use skillbank_core::agents::AgentId;
use skillbank_core::skill::validate::{
    Profile, Severity, ValidateOptions, ValidationReport, validate_dir, validate_text,
};
use std::fs;

const BODY: &str = "\n# Título\n\nInstrucciones paso a paso.\n";

fn md(fm: &str) -> String {
    format!("---\n{fm}\n---\n{BODY}")
}

fn base(extra: &str) -> String {
    md(&format!("name: mi-skill\ndescription: Hace algo útil. Úsala cuando el usuario lo pida.\n{extra}"))
}

fn run(text: &str, profile: Profile) -> ValidationReport {
    validate_text(text, &ValidateOptions { profile, dir_name: Some("mi-skill".into()), ..Default::default() })
}

fn sev_of(r: &ValidationReport, code: &str) -> Option<Severity> {
    r.diagnostics.iter().find(|d| d.code == code).map(|d| d.severity)
}

/// (descripción, texto, perfil, código esperado, severidad esperada)
fn cases() -> Vec<(&'static str, String, Profile, &'static str, Severity)> {
    use Profile::*;
    use Severity::*;
    let long_name = "a".repeat(65);
    let desc_1025 = format!("Úsala cuando {}", "x".repeat(1025 - 13));
    vec![
        ("sin frontmatter", "# Hola\ntexto".into(), Open, "E_FM_MISSING", Error),
        ("frontmatter sin cerrar", "---\nname: mi-skill\ndescription: x\n".into(), Open, "E_FM_UNCLOSED", Error),
        ("yaml inválido", md("name: [mi-skill\ndescription: x"), Open, "E_FM_YAML", Error),
        (
            "dos puntos sin comillas",
            md("name: mi-skill\ndescription: Hace algo: muy útil: sí"),
            Open,
            "E_FM_YAML",
            Error,
        ),
        ("frontmatter es lista", md("- a\n- b"), Open, "E_FM_NOT_MAP", Error),
        ("frontmatter vacío: falta name", md(""), Open, "E_NAME_MISSING", Error),
        ("frontmatter vacío: falta description", md(""), Open, "E_DESC_MISSING", Error),
        ("falta name", md("description: x cuando"), Open, "E_NAME_MISSING", Error),
        ("name nulo", md("name:\ndescription: x"), Open, "E_NAME_MISSING", Error),
        ("name booleano", md("name: true\ndescription: x"), Open, "E_NAME_TYPE", Error),
        ("name numérico", md("name: 123\ndescription: x"), Open, "E_NAME_TYPE", Error),
        ("name con mayúsculas", md("name: Mi-Skill\ndescription: x"), Open, "E_NAME_CHARS", Error),
        ("name con guion bajo", md("name: mi_skill\ndescription: x"), Open, "E_NAME_CHARS", Error),
        ("name con espacio", md("name: \"mi skill\"\ndescription: x"), Open, "E_NAME_CHARS", Error),
        ("name con acento", md("name: revisión\ndescription: x"), Open, "E_NAME_CHARS", Error),
        ("name empieza con guion", md("name: -mi-skill\ndescription: x"), Open, "E_NAME_HYPHEN", Error),
        ("name termina con guion", md("name: mi-skill-\ndescription: x"), Open, "E_NAME_HYPHEN", Error),
        ("name con doble guion", md("name: mi--skill\ndescription: x"), Open, "E_NAME_HYPHEN", Error),
        ("name de 65", md(&format!("name: {long_name}\ndescription: x")), Open, "E_NAME_LEN", Error),
        ("name vacío", md("name: \"\"\ndescription: x"), Open, "E_NAME_LEN", Error),
        ("name distinto de la carpeta", md("name: otra-cosa\ndescription: x"), Open, "E_NAME_DIR_MISMATCH", Error),
        ("reservado synced", md("name: synced\ndescription: x"), Open, "E_NAME_RESERVED", Error),
        ("reservado Synced (mayúsculas)", md("name: Synced\ndescription: x"), Open, "E_NAME_RESERVED", Error),
        ("reservado anthropic-skills", md("name: anthropic-skills\ndescription: x"), Open, "E_NAME_RESERVED", Error),
        ("reservado banco-de-skills", md("name: banco-de-skills\ndescription: x"), Open, "E_NAME_RESERVED", Error),
        ("palabra claude (open)", md("name: claude-helper\ndescription: x"), Open, "C_RESERVED_WORD", Warning),
        ("palabra claude (claude-ai)", md("name: claude-helper\ndescription: x"), ClaudeAi, "C_RESERVED_WORD", Error),
        ("palabra anthropic", md("name: anthropic-tools\ndescription: x"), Open, "C_RESERVED_WORD", Warning),
        ("falta description", md("name: mi-skill"), Open, "E_DESC_MISSING", Error),
        ("description vacía", md("name: mi-skill\ndescription: \"\""), Open, "E_DESC_EMPTY", Error),
        ("description nula", md("name: mi-skill\ndescription:"), Open, "E_DESC_EMPTY", Error),
        ("description en blanco", md("name: mi-skill\ndescription: \"   \""), Open, "E_DESC_EMPTY", Error),
        ("description numérica", md("name: mi-skill\ndescription: 42"), Open, "E_DESC_TYPE", Error),
        ("description de 1025", md(&format!("name: mi-skill\ndescription: {desc_1025}")), Open, "E_DESC_LEN", Error),
        (
            "description con XML (open)",
            base("").replace("Hace algo útil.", "Usa <tool> para todo."),
            Open,
            "C_XML_TAGS",
            Warning,
        ),
        (
            "description con XML (claude-ai)",
            base("").replace("Hace algo útil.", "Usa <tool> para todo."),
            ClaudeAi,
            "C_XML_TAGS",
            Error,
        ),
        (
            "description sin cuándo",
            md("name: mi-skill\ndescription: Procesa archivos PDF."),
            Open,
            "I_DESC_NO_WHEN",
            Info,
        ),
        ("license numérica", base("license: 3"), Open, "E_LICENSE_TYPE", Error),
        ("compatibility de 501", base(&format!("compatibility: {}", "c".repeat(501))), Open, "E_COMPAT_LEN", Error),
        ("compatibility lista", base("compatibility:\n  - linux"), Open, "E_COMPAT_TYPE", Error),
        ("metadata texto", base("metadata: hola"), Open, "E_METADATA_TYPE", Error),
        ("metadata con número", base("metadata:\n  version: 1.0"), Open, "W_METADATA_VALUE", Warning),
        ("allowed-tools lista", base("allowed-tools:\n  - Read\n  - Grep"), Open, "W_ALLOWED_TOOLS_LIST", Warning),
        ("allowed-tools lista (claude-ai)", base("allowed-tools:\n  - Read"), ClaudeAi, "W_ALLOWED_TOOLS_LIST", Error),
        ("allowed-tools número", base("allowed-tools: 5"), Open, "E_ALLOWED_TOOLS_TYPE", Error),
        ("campo de Claude Code (open)", base("model: sonnet"), Open, "I_CC_ONLY", Info),
        (
            "campo de Claude Code (claude-ai)",
            base("disable-model-invocation: true"),
            ClaudeAi,
            "C_NONSTANDARD_KEY",
            Error,
        ),
        ("campo desconocido", base("icono: 🚀"), Open, "W_UNKNOWN_KEY", Warning),
        ("campo desconocido (claude-ai)", base("icono: x"), ClaudeAi, "C_NONSTANDARD_KEY", Error),
        (
            "listado de Claude Code truncado",
            base(&format!("when_to_use: \"{}\"", "w".repeat(1500))),
            Open,
            "W_CC_LISTING_TRUNC",
            Warning,
        ),
        ("BOM", format!("\u{feff}{}", base("")), Open, "W_BOM", Warning),
        ("cuerpo vacío", "---\nname: mi-skill\ndescription: x cuando\n---\n".into(), Open, "W_BODY_EMPTY", Warning),
        (
            "cuerpo de 501 líneas",
            format!("---\nname: mi-skill\ndescription: x cuando\n---\n{}", "línea\n".repeat(501)),
            Open,
            "W_BODY_LINES",
            Warning,
        ),
        (
            "cuerpo de más de 5000 tokens",
            format!("---\nname: mi-skill\ndescription: x cuando\n---\n{}", "palabra ".repeat(3000)),
            Open,
            "W_BODY_TOKENS",
            Warning,
        ),
    ]
}

#[test]
fn tabla_de_casos() {
    let mut failures = Vec::new();
    let all = cases();
    assert!(all.len() >= 45, "se esperan al menos 45 casos, hay {}", all.len());
    for (desc, text, profile, code, sev) in all {
        let r = run(&text, profile);
        match sev_of(&r, code) {
            Some(s) if s == sev => {}
            other => {
                failures.push(format!("[{desc}] esperaba {code} ({sev:?}), obtuve {other:?}; códigos: {:?}", r.codes()))
            }
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

#[test]
fn skills_validas_sin_errores() {
    let ok = [
        base(""),
        md(&format!("name: {}\ndescription: Úsala cuando haga falta.", "a".repeat(64))),
        md(&format!("name: mi-skill\ndescription: \"Úsala cuando {}\"", "y".repeat(1024 - 13))),
        base("license: MIT\ncompatibility: Requiere Python 3\nmetadata:\n  autor: yo\nallowed-tools: Read Grep"),
        base("").replace('\n', "\r\n"),
        format!("{}\n---\nSeparador en el cuerpo\n", base("")),
        base("").trim_end().to_string(),
    ];
    for (i, text) in ok.iter().enumerate() {
        // Sin comparar con la carpeta: aquí solo interesan las reglas del contenido.
        let r = validate_text(text, &ValidateOptions::default());
        assert!(!r.has_errors(), "caso {i}: {:?}", r.diagnostics);
    }
}

#[test]
fn perfil_claude_code_no_avisa_campos_propios() {
    let r = run(&base("model: sonnet\ncontext: fork"), Profile::ClaudeCode);
    assert!(r.diagnostics.iter().all(|d| d.code != "I_CC_ONLY"), "{:?}", r.codes());
    assert_eq!(r.claude_code_fields, vec!["model", "context"]);
}

#[test]
fn campo_cc_con_agente_destino() {
    let opts = |agents: Vec<AgentId>| ValidateOptions {
        dir_name: Some("mi-skill".into()),
        target_agents: agents,
        ..Default::default()
    };
    let r = validate_text(&base("model: sonnet"), &opts(vec![AgentId::Antigravity]));
    assert_eq!(sev_of(&r, "W_CC_ONLY_TARGET"), Some(Severity::Warning));
    // Cursor sí entiende `paths`.
    let r = validate_text(&base("paths: src/**"), &opts(vec![AgentId::Cursor]));
    assert_eq!(sev_of(&r, "W_CC_ONLY_TARGET"), None);
    assert_eq!(sev_of(&r, "I_CC_ONLY"), Some(Severity::Info));
}

#[test]
fn discrepancia_de_carpeta_como_aviso() {
    let r = validate_text(
        &md("name: otra-cosa\ndescription: x cuando"),
        &ValidateOptions { dir_name: Some("mi-skill".into()), dir_mismatch_is_warning: true, ..Default::default() },
    );
    assert_eq!(sev_of(&r, "E_NAME_DIR_MISMATCH"), Some(Severity::Warning));
}

#[test]
fn numeros_de_linea() {
    let text = "---\ndescription: x cuando\nname: Mal_Nombre\n---\ncuerpo\n";
    let r = run(text, Profile::Open);
    let d = r.diagnostics.iter().find(|d| d.code == "E_NAME_CHARS").unwrap();
    assert_eq!(d.line, Some(3));
    assert!(d.hint.as_deref().unwrap().contains("mal-nombre"));
}

#[test]
fn claves_duplicadas_son_error() {
    let r = run(&md("name: mi-skill\nname: otra\ndescription: x"), Profile::Open);
    assert!(r.has_errors(), "{:?}", r.codes());
}

// ───── validaciones que miran la carpeta ─────

fn skill_dir(files: &[(&str, &[u8])]) -> tempfile::TempDir {
    let t = tempfile::tempdir().unwrap();
    let root = t.path().join("mi-skill");
    fs::create_dir(&root).unwrap();
    for (rel, content) in files {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, content).unwrap();
    }
    t
}

fn run_dir(t: &tempfile::TempDir) -> ValidationReport {
    validate_dir(&t.path().join("mi-skill"), &ValidateOptions::default())
}

#[test]
fn carpeta_sin_skill_md() {
    let t = skill_dir(&[("README.md", b"x")]);
    assert_eq!(run_dir(&t).codes(), vec!["E_SKILLMD_MISSING"]);
}

#[test]
fn carpeta_con_skill_md_en_minusculas() {
    let t = skill_dir(&[("skill.md", base("").as_bytes())]);
    let r = run_dir(&t);
    assert_eq!(r.codes(), vec!["E_SKILLMD_MISSING"]);
    // En sistemas de archivos que distinguen mayúsculas, la pista menciona skill.md.
    assert!(r.diagnostics[0].hint.as_deref().unwrap().contains("SKILL.md"));
}

#[test]
fn skill_md_no_utf8() {
    let t = skill_dir(&[("SKILL.md", &[0xff, 0xfe, 0x00, 0x41])]);
    assert_eq!(run_dir(&t).codes(), vec!["E_UTF8"]);
}

#[test]
fn enlaces_rotos_barras_y_anidados() {
    let skill =
        format!("{}\nVer [ref](references/a.md), [rota](references/no.md) y [win](references\\b.md).\n", base(""));
    let t = skill_dir(&[
        ("SKILL.md", skill.as_bytes()),
        ("references/a.md", b"# A\nVer [c](c.md)\n"),
        ("references/c.md", b"# C\n"),
    ]);
    let r = run_dir(&t);
    let codes = r.codes();
    assert!(codes.contains(&"W_BROKEN_LINK"), "{codes:?}");
    assert!(codes.contains(&"W_BACKSLASH_PATH"), "{codes:?}");
    assert!(codes.contains(&"W_NESTED_REFS"), "{codes:?}");
    assert!(!r.has_errors());
}

#[test]
fn referencia_larga_sin_indice() {
    let long = "texto\n".repeat(120);
    let with_toc = format!("# Ref\n\n## Contenido\n- [a](#a)\n{long}");
    let t = skill_dir(&[
        ("SKILL.md", base("").as_bytes()),
        ("references/larga.md", long.as_bytes()),
        ("references/con-indice.md", with_toc.as_bytes()),
    ]);
    let r = run_dir(&t);
    let tocs: Vec<_> = r.diagnostics.iter().filter(|d| d.code == "W_REF_NO_TOC").map(|d| d.file.clone()).collect();
    assert_eq!(tocs, vec!["references/larga.md"]);
}

#[test]
fn archivo_de_conflicto() {
    let t = skill_dir(&[("SKILL.md", base("").as_bytes()), ("SKILL.md.sbconflict1", b"x")]);
    assert!(run_dir(&t).codes().contains(&"W_CONFLICT_FILE"));
}
