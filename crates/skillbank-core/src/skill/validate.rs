//! Validador de skills según la especificación abierta Agent Skills (agentskills.io)
//! y las reglas de la plataforma Claude. Un solo motor para CLI y app.
//! Los códigos de diagnóstico están documentados en docs/spec-validator.md.

use super::frontmatter::{self, key_lines, parse_yaml, split, type_name};
use super::links;
use crate::agents::AgentId;
use crate::text::{fold, has_xml_tag, slugify};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::Path;
use yaml_serde::Value;

/// Las 6 claves del estándar (las únicas que aceptan claude.ai y la API).
pub const STANDARD_KEYS: &[&str] = &["name", "description", "license", "compatibility", "metadata", "allowed-tools"];

/// Claves que solo entiende Claude Code.
pub const CLAUDE_CODE_KEYS: &[&str] = &[
    "when_to_use",
    "argument-hint",
    "arguments",
    "disable-model-invocation",
    "user-invocable",
    "disallowed-tools",
    "model",
    "effort",
    "context",
    "agent",
    "background",
    "hooks",
    "paths",
    "shell",
];

/// Nombres reservados (sin distinguir mayúsculas).
pub const RESERVED_NAMES: &[&str] = &["synced", "anthropic-skills", "banco-de-skills"];

pub const TAG_CLAUDE_CODE: &str = "solo Claude Code";

pub const MAX_NAME: usize = 64;
pub const MAX_DESCRIPTION: usize = 1024;
pub const MAX_COMPATIBILITY: usize = 500;
pub const MAX_CC_LISTING: usize = 1536;
pub const MAX_BODY_LINES: usize = 500;
pub const MAX_BODY_TOKENS: usize = 5000;
pub const REF_TOC_LINES: usize = 100;
pub const MAX_FILE_BYTES: u64 = 5 * 1024 * 1024;
pub const MAX_SKILL_BYTES: u64 = 20 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Profile {
    /// Estándar abierto (por defecto).
    #[default]
    Open,
    /// Estricto: lo que acepta la subida a claude.ai / Skills API.
    ClaudeAi,
    /// Claude Code: los campos propios no generan avisos.
    ClaudeCode,
}

impl std::str::FromStr for Profile {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "open" | "estandar" | "estándar" => Ok(Profile::Open),
            "claude-ai" | "claudeai" | "api" => Ok(Profile::ClaudeAi),
            "claude-code" | "claudecode" => Ok(Profile::ClaudeCode),
            _ => Err(format!("perfil desconocido '{s}' (usa open, claude-ai o claude-code)")),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    pub code: String,
    pub severity: Severity,
    pub message: String,
    pub file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ValidateOptions {
    pub profile: Profile,
    /// Nombre de carpeta con el que debe coincidir `name`. `None` = no comparar.
    pub dir_name: Option<String>,
    /// La discrepancia nombre/carpeta es aviso en vez de error (al subir, el banco guarda por `name`).
    pub dir_mismatch_is_warning: bool,
    /// Agentes destino: los campos solo-Claude-Code se vuelven avisos si el destino no los entiende.
    pub target_agents: Vec<AgentId>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ValidationReport {
    pub name: Option<String>,
    pub description: Option<String>,
    /// Frontmatter como JSON (null si no se pudo leer).
    pub frontmatter: serde_json::Value,
    pub body_lines: usize,
    pub body_tokens_est: usize,
    /// Campos presentes que solo entiende Claude Code.
    pub claude_code_fields: Vec<String>,
    pub diagnostics: Vec<Diagnostic>,
}

impl ValidationReport {
    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(|d| d.severity == Severity::Error)
    }
    pub fn count(&self, sev: Severity) -> usize {
        self.diagnostics.iter().filter(|d| d.severity == sev).count()
    }
    pub fn codes(&self) -> Vec<&str> {
        self.diagnostics.iter().map(|d| d.code.as_str()).collect()
    }
    pub fn errors(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics.iter().filter(|d| d.severity == Severity::Error)
    }
    pub fn warnings(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics.iter().filter(|d| d.severity == Severity::Warning)
    }
}

struct Ctx<'a> {
    opts: &'a ValidateOptions,
    file: String,
    lines: BTreeMap<String, usize>,
    out: Vec<Diagnostic>,
}

impl Ctx<'_> {
    fn push(&mut self, code: &str, sev: Severity, field: Option<&str>, message: String, hint: Option<String>) {
        let line = field.and_then(|f| self.lines.get(f).copied());
        self.out.push(Diagnostic {
            code: code.to_string(),
            severity: sev,
            message,
            file: self.file.clone(),
            line,
            column: None,
            field: field.map(str::to_string),
            tag: None,
            hint,
        });
    }
    fn tagged(&mut self, code: &str, sev: Severity, field: &str, message: String, tag: &str, hint: Option<String>) {
        self.push(code, sev, Some(field), message, hint);
        if let Some(d) = self.out.last_mut() {
            d.tag = Some(tag.to_string());
        }
    }
    /// Error en claude-ai, aviso en los demás perfiles.
    fn strict_sev(&self) -> Severity {
        if self.opts.profile == Profile::ClaudeAi { Severity::Error } else { Severity::Warning }
    }
}

/// Valida el texto de un SKILL.md (sin mirar otros archivos). Útil para borradores del editor.
pub fn validate_text(skill_md: &str, opts: &ValidateOptions) -> ValidationReport {
    let mut report = ValidationReport::default();
    let sp = split(skill_md);
    let mut cx = Ctx { opts, file: "SKILL.md".into(), lines: BTreeMap::new(), out: vec![] };

    if sp.had_bom {
        cx.push(
            "W_BOM",
            Severity::Warning,
            None,
            "SKILL.md empieza con una marca BOM (UTF-8 con BOM).".into(),
            Some("Guárdalo como UTF-8 sin BOM.".into()),
        );
    }

    let Some(yaml) = sp.yaml.as_deref() else {
        cx.out.push(Diagnostic {
            code: "E_FM_MISSING".into(),
            severity: Severity::Error,
            message: "SKILL.md no tiene frontmatter: la primera línea debe ser exactamente `---`.".into(),
            file: cx.file.clone(),
            line: Some(1),
            column: None,
            field: None,
            tag: None,
            hint: Some(
                "Empieza el archivo con:\n---\nname: mi-skill\ndescription: Qué hace y cuándo usarla.\n---".into(),
            ),
        });
        body_checks(&sp.body, &mut cx, &mut report);
        report.diagnostics = cx.out;
        return report;
    };

    if sp.unclosed {
        cx.out.push(Diagnostic {
            code: "E_FM_UNCLOSED".into(),
            severity: Severity::Error,
            message: "El frontmatter no está cerrado: falta la línea `---` final.".into(),
            file: cx.file.clone(),
            line: Some(1),
            column: None,
            field: None,
            tag: None,
            hint: None,
        });
        report.diagnostics = cx.out;
        return report;
    }

    cx.lines = key_lines(yaml, sp.yaml_first_line);
    let value = match parse_yaml(yaml, sp.yaml_first_line) {
        Ok(v) => v,
        Err(e) => {
            let mut hint = None;
            if e.message.contains("mapping values are not allowed") || e.message.contains("did not find expected key") {
                hint = Some("Si la descripción contiene `:`, ponla entre comillas: description: \"…\"".into());
            }
            cx.out.push(Diagnostic {
                code: "E_FM_YAML".into(),
                severity: Severity::Error,
                message: format!("El frontmatter no es YAML válido: {}", e.message),
                file: cx.file.clone(),
                line: e.line,
                column: e.column,
                field: None,
                tag: None,
                hint,
            });
            body_checks(&sp.body, &mut cx, &mut report);
            report.diagnostics = cx.out;
            return report;
        }
    };

    let map = match value {
        Value::Mapping(m) => m,
        Value::Null => Default::default(),
        other => {
            cx.push(
                "E_FM_NOT_MAP",
                Severity::Error,
                None,
                format!("El frontmatter debe ser un mapa de claves (es {}).", type_name(&other)),
                None,
            );
            report.diagnostics = cx.out;
            return report;
        }
    };
    report.frontmatter = frontmatter::to_json(&Value::Mapping(map.clone()));

    let get = |k: &str| map.get(Value::String(k.into()));

    // --- name ---
    match get("name") {
        None => cx.push(
            "E_NAME_MISSING",
            Severity::Error,
            None,
            "Falta el campo obligatorio `name`.".into(),
            opts.dir_name.as_ref().map(|d| format!("Añade: name: {}", slugify(d))),
        ),
        Some(Value::Null) => {
            cx.push("E_NAME_MISSING", Severity::Error, Some("name"), "El campo `name` está vacío.".into(), None)
        }
        Some(Value::String(name)) => {
            report.name = Some(name.clone());
            check_name(name, &mut cx);
        }
        Some(other) => cx.push(
            "E_NAME_TYPE",
            Severity::Error,
            Some("name"),
            format!("`name` debe ser texto (es {}).", type_name(other)),
            Some("Ponlo entre comillas, p. ej. name: \"no\".".into()),
        ),
    }

    // --- description ---
    let mut desc_len = 0usize;
    match get("description") {
        None => cx.push(
            "E_DESC_MISSING",
            Severity::Error,
            None,
            "Falta el campo obligatorio `description`.".into(),
            Some("Describe qué hace la skill y cuándo usarla (máx. 1024 caracteres).".into()),
        ),
        Some(Value::Null) => {
            cx.push("E_DESC_EMPTY", Severity::Error, Some("description"), "La descripción está vacía.".into(), None)
        }
        Some(Value::String(d)) => {
            report.description = Some(d.clone());
            desc_len = d.chars().count();
            if d.trim().is_empty() {
                cx.push(
                    "E_DESC_EMPTY",
                    Severity::Error,
                    Some("description"),
                    "La descripción está vacía.".into(),
                    None,
                );
            } else {
                if desc_len > MAX_DESCRIPTION {
                    cx.push(
                        "E_DESC_LEN",
                        Severity::Error,
                        Some("description"),
                        format!("La descripción tiene {desc_len} caracteres (máximo {MAX_DESCRIPTION})."),
                        Some("Resume y mueve los detalles al cuerpo de SKILL.md.".into()),
                    );
                }
                if has_xml_tag(d) {
                    let sev = cx.strict_sev();
                    cx.push(
                        "C_XML_TAGS",
                        sev,
                        Some("description"),
                        "La descripción contiene etiquetas XML (no permitidas en la plataforma Claude).".into(),
                        None,
                    );
                }
                if !mentions_when(d) {
                    cx.push(
                        "I_DESC_NO_WHEN",
                        Severity::Info,
                        Some("description"),
                        "La descripción no parece indicar cuándo usar la skill.".into(),
                        Some("Añade algo como \"Úsala cuando…\" para que el agente sepa activarla.".into()),
                    );
                }
            }
        }
        Some(other) => cx.push(
            "E_DESC_TYPE",
            Severity::Error,
            Some("description"),
            format!("`description` debe ser texto (es {}).", type_name(other)),
            None,
        ),
    }

    // --- license ---
    if let Some(v) = get("license")
        && !matches!(v, Value::String(_))
    {
        cx.push(
            "E_LICENSE_TYPE",
            Severity::Error,
            Some("license"),
            format!("`license` debe ser texto (es {}).", type_name(v)),
            None,
        );
    }

    // --- compatibility ---
    if let Some(v) = get("compatibility") {
        match v {
            Value::String(s) => {
                let n = s.chars().count();
                if n == 0 || n > MAX_COMPATIBILITY {
                    cx.push(
                        "E_COMPAT_LEN",
                        Severity::Error,
                        Some("compatibility"),
                        format!("`compatibility` debe tener entre 1 y {MAX_COMPATIBILITY} caracteres (tiene {n})."),
                        None,
                    );
                }
            }
            other => cx.push(
                "E_COMPAT_TYPE",
                Severity::Error,
                Some("compatibility"),
                format!("`compatibility` debe ser texto (es {}).", type_name(other)),
                None,
            ),
        }
    }

    // --- metadata ---
    if let Some(v) = get("metadata") {
        match v {
            Value::Mapping(m) => {
                for (k, val) in m {
                    let key_ok = matches!(k, Value::String(_));
                    let val_ok = matches!(val, Value::String(_));
                    if !key_ok || !val_ok {
                        let kname = k.as_str().map(str::to_string).unwrap_or_else(|| format!("{k:?}"));
                        cx.push(
                            "W_METADATA_VALUE",
                            Severity::Warning,
                            Some("metadata"),
                            format!("`metadata.{kname}` debería ser texto (el estándar usa pares texto→texto)."),
                            Some("Pon el valor entre comillas, p. ej. version: \"1.0\".".into()),
                        );
                    }
                }
            }
            Value::Null => {}
            other => cx.push(
                "E_METADATA_TYPE",
                Severity::Error,
                Some("metadata"),
                format!("`metadata` debe ser un mapa de texto a texto (es {}).", type_name(other)),
                None,
            ),
        }
    }

    // --- allowed-tools ---
    if let Some(v) = get("allowed-tools") {
        match v {
            Value::String(_) | Value::Null => {}
            Value::Sequence(_) => {
                let sev = if opts.profile == Profile::ClaudeAi { Severity::Error } else { Severity::Warning };
                cx.tagged(
                    "W_ALLOWED_TOOLS_LIST",
                    sev,
                    "allowed-tools",
                    "`allowed-tools` es una lista; el estándar usa un texto separado por espacios.".into(),
                    TAG_CLAUDE_CODE,
                    Some("Ejemplo: allowed-tools: Read Grep Bash(git:*)".into()),
                );
            }
            other => cx.push(
                "E_ALLOWED_TOOLS_TYPE",
                Severity::Error,
                Some("allowed-tools"),
                format!("`allowed-tools` debe ser texto (es {}).", type_name(other)),
                None,
            ),
        }
    }

    // --- claves no estándar ---
    let non_claude_targets: Vec<AgentId> =
        opts.target_agents.iter().copied().filter(|a| *a != AgentId::Claude).collect();
    for k in map.keys() {
        let Some(key) = k.as_str() else {
            cx.push("E_FM_KEY_TYPE", Severity::Error, None, format!("Clave de frontmatter no textual: {k:?}"), None);
            continue;
        };
        if STANDARD_KEYS.contains(&key) {
            continue;
        }
        if opts.profile == Profile::ClaudeAi {
            cx.push(
                "C_NONSTANDARD_KEY",
                Severity::Error,
                Some(key),
                format!("claude.ai y la API solo aceptan los 6 campos estándar; `{key}` haría fallar la subida."),
                Some("Exporta con --strip-nonstandard para quitarlo solo en la copia exportada.".into()),
            );
            continue;
        }
        if CLAUDE_CODE_KEYS.contains(&key) {
            report.claude_code_fields.push(key.to_string());
            let unaware: Vec<&str> =
                non_claude_targets.iter().filter(|a| !a.understands_field(key)).map(|a| a.display()).collect();
            if unaware.is_empty() {
                if opts.profile != Profile::ClaudeCode {
                    cx.tagged(
                        "I_CC_ONLY",
                        Severity::Info,
                        key,
                        format!("`{key}` solo lo entiende Claude Code."),
                        TAG_CLAUDE_CODE,
                        None,
                    );
                }
            } else {
                cx.tagged(
                    "W_CC_ONLY_TARGET",
                    Severity::Warning,
                    key,
                    format!("`{key}` solo lo entiende Claude Code; {} lo ignorará.", unaware.join(", ")),
                    TAG_CLAUDE_CODE,
                    None,
                );
            }
        } else {
            cx.push(
                "W_UNKNOWN_KEY",
                Severity::Warning,
                Some(key),
                format!("Campo desconocido `{key}`: no es del estándar ni de Claude Code."),
                None,
            );
        }
    }

    // --- límite del listado de Claude Code ---
    if let Some(Value::String(w)) = get("when_to_use") {
        let total = desc_len + w.chars().count();
        if total > MAX_CC_LISTING {
            cx.push(
                "W_CC_LISTING_TRUNC",
                Severity::Warning,
                Some("when_to_use"),
                format!("description + when_to_use suman {total} caracteres; Claude Code corta a {MAX_CC_LISTING}."),
                None,
            );
        }
    }

    body_checks(&sp.body, &mut cx, &mut report);
    report.diagnostics = cx.out;
    report
}

fn check_name(name: &str, cx: &mut Ctx) {
    let n = name.chars().count();
    if n == 0 || n > MAX_NAME {
        cx.push(
            "E_NAME_LEN",
            Severity::Error,
            Some("name"),
            format!("`name` debe tener entre 1 y {MAX_NAME} caracteres (tiene {n})."),
            None,
        );
    }
    if !name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
        let suggestion = slugify(name);
        cx.push(
            "E_NAME_CHARS",
            Severity::Error,
            Some("name"),
            "`name` solo admite minúsculas (a-z), números y guiones.".into(),
            (!suggestion.is_empty()).then(|| format!("Sugerencia: name: {suggestion}")),
        );
    }
    if name.starts_with('-') || name.ends_with('-') || name.contains("--") {
        cx.push(
            "E_NAME_HYPHEN",
            Severity::Error,
            Some("name"),
            "`name` no puede empezar ni terminar con guion ni tener guiones seguidos (`--`).".into(),
            None,
        );
    }
    let lower = name.to_lowercase();
    if RESERVED_NAMES.contains(&lower.as_str()) || lower.starts_with("anthropic-skills:") {
        cx.push("E_NAME_RESERVED", Severity::Error, Some("name"), format!("`{name}` es un nombre reservado."), None);
    } else if lower.contains("anthropic") || lower.contains("claude") {
        let sev = cx.strict_sev();
        cx.push(
            "C_RESERVED_WORD",
            sev,
            Some("name"),
            "`name` contiene una palabra reservada por la plataforma Claude (\"anthropic\" o \"claude\").".into(),
            Some("Funciona en Claude Code, pero claude.ai y la API la rechazan.".into()),
        );
    }
    if has_xml_tag(name) {
        let sev = cx.strict_sev();
        cx.push("C_XML_TAGS", sev, Some("name"), "`name` contiene etiquetas XML.".into(), None);
    }
    if let Some(dir) = cx.opts.dir_name.clone()
        && dir != name
    {
        let sev = if cx.opts.dir_mismatch_is_warning { Severity::Warning } else { Severity::Error };
        cx.push(
            "E_NAME_DIR_MISMATCH",
            sev,
            Some("name"),
            format!("`name` (\"{name}\") no coincide con el nombre de la carpeta (\"{dir}\")."),
            Some("El estándar exige que sean iguales. Renombra la carpeta o el campo.".into()),
        );
    }
}

fn mentions_when(desc: &str) -> bool {
    let f = fold(desc);
    [
        "cuando",
        "when",
        "usala",
        "usar para",
        "use for",
        "use it",
        "use this",
        "useful",
        "util para",
        "si el usuario",
        "if the user",
        "trigger",
        "whenever",
        "siempre que",
    ]
    .iter()
    .any(|k| f.contains(k))
}

fn body_checks(body: &str, cx: &mut Ctx, report: &mut ValidationReport) {
    let trimmed = body.trim_end();
    report.body_lines = if trimmed.trim().is_empty() { 0 } else { trimmed.lines().count() };
    report.body_tokens_est = body.chars().count().div_ceil(4);
    if trimmed.trim().is_empty() {
        cx.push(
            "W_BODY_EMPTY",
            Severity::Warning,
            None,
            "El cuerpo de SKILL.md está vacío: el agente no tendrá instrucciones.".into(),
            None,
        );
    }
    if report.body_lines > MAX_BODY_LINES {
        cx.push(
            "W_BODY_LINES",
            Severity::Warning,
            None,
            format!("El cuerpo tiene {} líneas (recomendado: menos de {MAX_BODY_LINES}).", report.body_lines),
            Some("Mueve el material de referencia a references/ y enlázalo.".into()),
        );
    }
    if report.body_tokens_est > MAX_BODY_TOKENS {
        cx.push(
            "W_BODY_TOKENS",
            Severity::Warning,
            None,
            format!("El cuerpo tiene ≈{} tokens (recomendado: menos de {MAX_BODY_TOKENS}).", report.body_tokens_est),
            None,
        );
    }
}

/// Valida una carpeta de skill completa (SKILL.md + archivos).
pub fn validate_dir(dir: &Path, opts: &ValidateOptions) -> ValidationReport {
    let mut opts = opts.clone();
    if opts.dir_name.is_none() {
        opts.dir_name = dir.file_name().map(|n| n.to_string_lossy().into_owned());
    }
    let skill_md = dir.join("SKILL.md");
    let bytes = match std::fs::read(&skill_md) {
        Ok(b) => b,
        Err(_) => {
            let lower = dir.join("skill.md");
            let hint = if lower.exists() {
                Some("Existe `skill.md`: renómbralo a `SKILL.md` (en mayúsculas).".to_string())
            } else {
                Some("Toda skill necesita un archivo SKILL.md en su carpeta raíz.".to_string())
            };
            return ValidationReport {
                diagnostics: vec![Diagnostic {
                    code: "E_SKILLMD_MISSING".into(),
                    severity: Severity::Error,
                    message: format!("No hay SKILL.md en {}", dir.display()),
                    file: "SKILL.md".into(),
                    line: None,
                    column: None,
                    field: None,
                    tag: None,
                    hint,
                }],
                ..Default::default()
            };
        }
    };
    let text = match String::from_utf8(bytes) {
        Ok(t) => t,
        Err(_) => {
            return ValidationReport {
                diagnostics: vec![Diagnostic {
                    code: "E_UTF8".into(),
                    severity: Severity::Error,
                    message: "SKILL.md no está codificado en UTF-8.".into(),
                    file: "SKILL.md".into(),
                    line: None,
                    column: None,
                    field: None,
                    tag: None,
                    hint: None,
                }],
                ..Default::default()
            };
        }
    };
    let mut report = validate_text(&text, &opts);
    let sp = split(&text);
    file_checks(dir, &sp.body, sp.body_first_line, &mut report.diagnostics);
    report
}

fn diag(
    code: &str,
    sev: Severity,
    file: &str,
    line: Option<usize>,
    message: String,
    hint: Option<String>,
) -> Diagnostic {
    Diagnostic {
        code: code.into(),
        severity: sev,
        message,
        file: file.into(),
        line,
        column: None,
        field: None,
        tag: None,
        hint,
    }
}

fn file_checks(dir: &Path, body: &str, body_first_line: usize, out: &mut Vec<Diagnostic>) {
    // Enlaces desde SKILL.md
    let mut referenced_md: Vec<String> = Vec::new();
    for link in links::extract_links(body) {
        if !links::is_local_relative(&link.target) {
            continue;
        }
        let line = Some(body_first_line + link.line - 1);
        if link.target.contains('\\') {
            out.push(diag(
                "W_BACKSLASH_PATH",
                Severity::Warning,
                "SKILL.md",
                line,
                format!("La ruta `{}` usa `\\`; usa `/` para que funcione en todos los sistemas.", link.target),
                None,
            ));
            continue;
        }
        let path = links::link_path(&link.target);
        if path.is_empty() {
            continue;
        }
        let abs = dir.join(&path);
        if !abs.exists() {
            out.push(diag(
                "W_BROKEN_LINK",
                Severity::Warning,
                "SKILL.md",
                line,
                format!("El enlace `{path}` apunta a un archivo que no existe."),
                None,
            ));
        } else if path.ends_with(".md") && abs.is_file() {
            referenced_md.push(path);
        }
    }

    // Referencias anidadas: SKILL.md → ref.md → otro.md
    for rel in &referenced_md {
        let Ok(text) = std::fs::read_to_string(dir.join(rel)) else { continue };
        let base = Path::new(rel).parent().unwrap_or(Path::new(""));
        for link in links::extract_links(&text) {
            if !links::is_local_relative(&link.target) {
                continue;
            }
            let p = links::link_path(&link.target);
            if !p.ends_with(".md") {
                continue;
            }
            let target = crate::paths::normalize(&base.join(&p));
            let target_s = target.to_string_lossy().replace('\\', "/");
            if target_s == "SKILL.md" {
                continue;
            }
            if dir.join(&target).is_file() {
                out.push(diag(
                    "W_NESTED_REFS",
                    Severity::Warning,
                    rel,
                    Some(link.line),
                    format!("`{rel}` enlaza a `{target_s}`: mantén las referencias a un solo nivel desde SKILL.md."),
                    None,
                ));
                break;
            }
        }
    }

    // Archivos: índices, tamaños y conflictos
    let mut total: u64 = 0;
    if let Ok(files) = crate::fsutil::list_files(dir) {
        for f in &files {
            total += f.size;
            if f.size > MAX_FILE_BYTES {
                out.push(diag(
                    "W_LARGE_FILE",
                    Severity::Warning,
                    &f.rel,
                    None,
                    format!("`{}` pesa {:.1} MB (más de 5 MB).", f.rel, f.size as f64 / 1_048_576.0),
                    None,
                ));
            }
            if f.rel != "SKILL.md"
                && f.rel.ends_with(".md")
                && f.size < MAX_FILE_BYTES
                && let Ok(t) = std::fs::read_to_string(&f.abs)
                && t.lines().count() > REF_TOC_LINES
                && !links::has_toc(&t)
            {
                out.push(diag(
                    "W_REF_NO_TOC",
                    Severity::Warning,
                    &f.rel,
                    None,
                    format!("`{}` tiene más de {REF_TOC_LINES} líneas y no tiene índice al inicio.", f.rel),
                    Some("Añade una sección \"## Contenido\" con enlaces a las secciones.".into()),
                ));
            }
        }
    }
    if total > MAX_SKILL_BYTES {
        out.push(diag(
            "W_LARGE_SKILL",
            Severity::Warning,
            "",
            None,
            format!("La skill pesa {:.1} MB en total (más de 20 MB).", total as f64 / 1_048_576.0),
            None,
        ));
    }
    for entry in walkdir::WalkDir::new(dir).into_iter().flatten() {
        let name = entry.file_name().to_string_lossy();
        if name.contains(".sbconflict") {
            let rel = entry.path().strip_prefix(dir).unwrap_or(entry.path()).to_string_lossy().into_owned();
            out.push(diag(
                "W_CONFLICT_FILE",
                Severity::Warning,
                &rel,
                None,
                format!("Hay un archivo de conflicto de sincronización: `{rel}`."),
                Some("Resuélvelo con `skillbank sync --conflicts`.".into()),
            ));
        }
    }
}
