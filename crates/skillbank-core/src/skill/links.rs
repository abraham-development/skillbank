//! Análisis simple de enlaces Markdown (sin dependencias): ignora bloques de código.

/// Enlace relativo encontrado en un Markdown.
#[derive(Debug, Clone, PartialEq)]
pub struct Link {
    pub target: String,
    /// Línea (1-based) dentro del texto analizado.
    pub line: usize,
}

/// Extrae los destinos de enlaces `[texto](destino)` e imágenes `![alt](destino)`.
pub fn extract_links(markdown: &str) -> Vec<Link> {
    let mut out = Vec::new();
    let mut in_fence = false;
    for (i, line) in markdown.lines().enumerate() {
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        let mut rest = line;
        while let Some(pos) = rest.find("](") {
            let after = &rest[pos + 2..];
            let Some(end) = find_closing_paren(after) else { break };
            let mut target = after[..end].trim();
            // Quita un título opcional: (ruta "título")
            if let Some(sp) = target.find(" \"") {
                target = target[..sp].trim();
            }
            let target = target.trim_start_matches('<').trim_end_matches('>');
            if !target.is_empty() {
                out.push(Link { target: target.to_string(), line: i + 1 });
            }
            rest = &after[end..];
        }
    }
    out
}

fn find_closing_paren(s: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (i, c) in s.char_indices() {
        match c {
            '(' => depth += 1,
            ')' if depth == 0 => return Some(i),
            ')' => depth -= 1,
            _ => {}
        }
    }
    None
}

/// ¿Es un enlace a un archivo local relativo (no URL, no ancla, no absoluto)?
pub fn is_local_relative(target: &str) -> bool {
    !(target.contains("://")
        || target.starts_with("mailto:")
        || target.starts_with('#')
        || target.starts_with('/')
        || target.starts_with("data:"))
}

/// Ruta de archivo del enlace: sin ancla ni query y con `%20` decodificado.
pub fn link_path(target: &str) -> String {
    let p = target.split(['#', '?']).next().unwrap_or("");
    p.replace("%20", " ")
}

/// ¿Parece tener un índice en las primeras 30 líneas?
pub fn has_toc(markdown: &str) -> bool {
    let head: Vec<&str> = markdown.lines().take(30).collect();
    let anchor_links = head.iter().filter(|l| l.contains("](#")).count();
    if anchor_links >= 3 {
        return true;
    }
    head.iter().any(|l| {
        let t = l.trim_start();
        if !t.starts_with('#') {
            return false;
        }
        let f = crate::text::fold(t);
        ["contenido", "indice", "contents", "table of contents", "toc", "tabla de"].iter().any(|k| f.contains(k))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extrae_enlaces_y_omite_codigo() {
        let md =
            "Ver [guía](references/guia.md) y ![img](assets/a.png \"t\")\n```\n[no](x.md)\n```\n[web](https://x.y)";
        let links = extract_links(md);
        let targets: Vec<_> = links.iter().map(|l| l.target.as_str()).collect();
        assert_eq!(targets, vec!["references/guia.md", "assets/a.png", "https://x.y"]);
        assert!(!is_local_relative("https://x.y"));
        assert!(is_local_relative("references/guia.md"));
        assert_eq!(link_path("a%20b.md#sec"), "a b.md");
    }

    #[test]
    fn detecta_indice() {
        assert!(has_toc("# Ref\n\n## Contenido\n- a"));
        assert!(has_toc("- [a](#a)\n- [b](#b)\n- [c](#c)"));
        assert!(!has_toc("# Ref\ntexto"));
    }
}
