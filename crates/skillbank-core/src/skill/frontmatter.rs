//! Separa y analiza el frontmatter YAML de un SKILL.md.

use std::collections::BTreeMap;
use yaml_serde::Value;

#[derive(Debug, Clone)]
pub struct SplitSkillMd {
    pub had_bom: bool,
    /// Texto YAML entre los dos `---` (sin incluirlos). `None` si no hay frontmatter.
    pub yaml: Option<String>,
    /// Línea (1-based) del archivo donde empieza el YAML.
    pub yaml_first_line: usize,
    /// `true` si hay `---` inicial pero falta el de cierre.
    pub unclosed: bool,
    pub body: String,
    /// Línea (1-based) donde empieza el cuerpo.
    pub body_first_line: usize,
}

/// Divide un SKILL.md en frontmatter y cuerpo. Acepta BOM y finales de línea CRLF.
pub fn split(text: &str) -> SplitSkillMd {
    let had_bom = text.starts_with('\u{feff}');
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let lines: Vec<&str> = text.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l)).collect();
    if lines.first().map(|l| l.trim_end()) != Some("---") {
        return SplitSkillMd {
            had_bom,
            yaml: None,
            yaml_first_line: 0,
            unclosed: false,
            body: lines.join("\n"),
            body_first_line: 1,
        };
    }
    let close = lines.iter().enumerate().skip(1).find(|(_, l)| {
        let t = l.trim_end();
        t == "---" || t == "..."
    });
    match close {
        Some((idx, _)) => SplitSkillMd {
            had_bom,
            yaml: Some(lines[1..idx].join("\n")),
            yaml_first_line: 2,
            unclosed: false,
            body: lines[idx + 1..].join("\n"),
            body_first_line: idx + 2,
        },
        None => SplitSkillMd {
            had_bom,
            yaml: Some(lines[1..].join("\n")),
            yaml_first_line: 2,
            unclosed: true,
            body: String::new(),
            body_first_line: lines.len() + 1,
        },
    }
}

/// Mapa clave → línea (1-based en el archivo) para claves de primer nivel.
pub fn key_lines(yaml: &str, first_line: usize) -> BTreeMap<String, usize> {
    let mut out = BTreeMap::new();
    for (i, line) in yaml.split('\n').enumerate() {
        if line.starts_with(' ') || line.starts_with('\t') || line.starts_with('#') || line.starts_with('-') {
            continue;
        }
        if let Some(pos) = line.find(':') {
            let key = line[..pos].trim().trim_matches(|c| c == '"' || c == '\'');
            if !key.is_empty() && !key.contains(' ') {
                out.entry(key.to_string()).or_insert(first_line + i);
            }
        }
    }
    out
}

#[derive(Debug, Clone)]
pub struct YamlError {
    pub message: String,
    /// Línea (1-based) dentro del archivo, si se conoce.
    pub line: Option<usize>,
    pub column: Option<usize>,
}

/// Analiza el YAML del frontmatter. Un frontmatter vacío se trata como mapa vacío.
pub fn parse_yaml(yaml: &str, first_line: usize) -> Result<Value, YamlError> {
    if yaml.trim().is_empty() {
        return Ok(Value::Mapping(Default::default()));
    }
    yaml_serde::from_str::<Value>(yaml).map_err(|e| {
        let loc = e.location();
        YamlError {
            message: e.to_string(),
            line: loc.as_ref().map(|l| first_line + l.line().saturating_sub(1)),
            column: loc.as_ref().map(|l| l.column()),
        }
    })
}

/// Convierte un valor YAML a JSON (para mostrarlo o serializarlo).
pub fn to_json(v: &Value) -> serde_json::Value {
    serde_json::to_value(v).unwrap_or(serde_json::Value::Null)
}

/// Nombre legible del tipo YAML de un valor.
pub fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "vacío",
        Value::Bool(_) => "booleano",
        Value::Number(_) => "número",
        Value::String(_) => "texto",
        Value::Sequence(_) => "lista",
        Value::Mapping(_) => "mapa",
        Value::Tagged(_) => "valor etiquetado",
    }
}
