//! Salida: sobre JSON estable para agentes o texto en español para personas.

use serde::Serialize;
use serde_json::{Value, json};
use skillbank_core::CoreError;

/// Respuesta correcta de un comando.
pub struct Reply {
    /// Mensaje corto (va en el JSON y es la primera línea en modo humano).
    pub message: String,
    pub data: Value,
    pub warnings: Vec<Value>,
    /// Texto adicional para personas (tablas, listas…).
    pub human: String,
}

impl Reply {
    pub fn new(message: impl Into<String>, data: impl Serialize) -> Reply {
        Reply {
            message: message.into(),
            data: serde_json::to_value(data).unwrap_or(Value::Null),
            warnings: vec![],
            human: String::new(),
        }
    }
    pub fn human(mut self, text: impl Into<String>) -> Reply {
        self.human = text.into();
        self
    }
    pub fn warnings(mut self, w: Vec<Value>) -> Reply {
        self.warnings = w;
        self
    }
}

pub struct Printer {
    pub json: bool,
    pub quiet: bool,
}

impl Printer {
    pub fn ok(&self, command: &str, r: Reply) -> i32 {
        if self.json {
            let env = json!({
                "schema": 1,
                "ok": true,
                "command": command,
                "code": "OK",
                "exit_code": 0,
                "message": r.message,
                "data": r.data,
                "warnings": r.warnings,
            });
            println!("{}", serde_json::to_string_pretty(&env).unwrap());
        } else {
            if !r.message.is_empty() {
                println!("{}", r.message);
            }
            if !self.quiet && !r.human.is_empty() {
                println!("{}", r.human.trim_end());
            }
            if !self.quiet {
                for w in &r.warnings {
                    eprintln!("  ⚠ {}", w.get("message").and_then(Value::as_str).unwrap_or(""));
                }
            }
        }
        0
    }

    pub fn err(&self, command: &str, e: &CoreError) -> i32 {
        let code = e.kind.exit_code();
        if self.json {
            let env = json!({
                "schema": 1,
                "ok": false,
                "command": command,
                "code": e.kind.code(),
                "exit_code": code,
                "message": e.message,
                "hint": e.hint,
                "data": e.data,
                "warnings": [],
            });
            println!("{}", serde_json::to_string_pretty(&env).unwrap());
        } else {
            eprintln!("✗ {}", e.message);
            let details = render_error_data(&e.data);
            if !details.is_empty() {
                eprintln!("{}", details.trim_end());
            }
            if let Some(h) = &e.hint {
                eprintln!("→ {h}");
            }
        }
        code
    }
}

/// Muestra en texto los datos útiles de un error (diagnósticos, diferencias, candidatos).
fn render_error_data(data: &Value) -> String {
    let mut s = String::new();
    if let Some(diags) = data.get("diagnostics").and_then(Value::as_array) {
        s.push_str(&render_diagnostics(diags));
    }
    if let Some(diff) = data.get("diff") {
        s.push_str(&render_diff(diff));
    }
    if let Some(items) = data.get("items").and_then(Value::as_array) {
        for it in items {
            if it.get("action").and_then(Value::as_str) == Some("conflict") {
                s.push_str(&format!(
                    "  • {} ({})\n",
                    it.get("path").and_then(Value::as_str).unwrap_or(""),
                    it.get("reason").and_then(Value::as_str).unwrap_or("")
                ));
                if let Some(diff) = it.get("diff") {
                    s.push_str(&render_diff(diff));
                }
            }
        }
    }
    if let Some(c) = data.get("candidates").and_then(Value::as_array) {
        for p in c {
            s.push_str(&format!("  • {}\n", p.as_str().unwrap_or("")));
        }
    }
    s
}

pub fn render_diff(diff: &Value) -> String {
    let mut s = String::new();
    if let Some(files) = diff.get("files").and_then(Value::as_array) {
        for f in files {
            let st = match f.get("status").and_then(Value::as_str).unwrap_or("") {
                "added" => "+",
                "removed" => "-",
                _ => "~",
            };
            s.push_str(&format!(
                "    {st} {}  (+{} −{})\n",
                f.get("path").and_then(Value::as_str).unwrap_or(""),
                f.get("added").and_then(Value::as_u64).unwrap_or(0),
                f.get("removed").and_then(Value::as_u64).unwrap_or(0)
            ));
        }
    }
    if let Some(p) = diff.get("skill_md_patch").and_then(Value::as_str) {
        for line in p.lines() {
            s.push_str(&format!("      {line}\n"));
        }
        if diff.get("patch_truncated").and_then(Value::as_bool) == Some(true) {
            s.push_str("      … (recortado; usa --diff para verlo completo)\n");
        }
    }
    s
}

pub fn render_diagnostics(diags: &[Value]) -> String {
    let mut s = String::new();
    for d in diags {
        let sym = match d.get("severity").and_then(Value::as_str).unwrap_or("") {
            "error" => "✗",
            "warning" => "⚠",
            _ => "ℹ",
        };
        let file = d.get("file").and_then(Value::as_str).unwrap_or("");
        let loc = match d.get("line").and_then(Value::as_u64) {
            Some(l) => format!("{file}:{l}"),
            None => file.to_string(),
        };
        let tag = d.get("tag").and_then(Value::as_str).map(|t| format!(" [{t}]")).unwrap_or_default();
        s.push_str(&format!(
            "  {sym} {} {}{tag}  ({loc})\n",
            d.get("code").and_then(Value::as_str).unwrap_or(""),
            d.get("message").and_then(Value::as_str).unwrap_or("")
        ));
        if let Some(h) = d.get("hint").and_then(Value::as_str) {
            for (i, line) in h.lines().enumerate() {
                s.push_str(&format!("      {}{line}\n", if i == 0 { "→ " } else { "  " }));
            }
        }
    }
    s
}
