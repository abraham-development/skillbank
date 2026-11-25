//! Utilidades de texto: plegado de acentos para búsquedas y slugs.

/// Minúsculas sin acentos ("Revisión" → "revision").
pub fn fold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        for l in c.to_lowercase() {
            out.push(match l {
                'á' | 'à' | 'ä' | 'â' | 'ã' | 'å' => 'a',
                'é' | 'è' | 'ë' | 'ê' => 'e',
                'í' | 'ì' | 'ï' | 'î' => 'i',
                'ó' | 'ò' | 'ö' | 'ô' | 'õ' => 'o',
                'ú' | 'ù' | 'ü' | 'û' => 'u',
                'ñ' => 'n',
                'ç' => 'c',
                other => other,
            });
        }
    }
    out
}

/// Convierte un título en un nombre de skill válido ("Revisión de PRs" → "revision-de-prs").
pub fn slugify(s: &str) -> String {
    let folded = fold(s);
    let mut out = String::new();
    let mut last_dash = true;
    for c in folded.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out.chars().take(64).collect::<String>().trim_end_matches('-').to_string()
}

/// Distancia de edición (Levenshtein) para sugerencias "¿quisiste decir…?".
pub fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len() + 1];
    for i in 1..=a.len() {
        cur[0] = i;
        for j in 1..=b.len() {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// ¿Contiene algo que parezca una etiqueta XML/HTML (`<tag>`, `</tag>`)?
pub fn has_xml_tag(s: &str) -> bool {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'<' {
            let rest = &s[i + 1..];
            let mut chars = rest.chars();
            let first = chars.next();
            let starts_tag = match first {
                Some('/') => chars.next().is_some_and(|c| c.is_ascii_alphabetic()),
                Some(c) => c.is_ascii_alphabetic(),
                None => false,
            };
            if starts_tag && let Some(end) = rest.find('>') {
                let inner = &rest[..end];
                if !inner.contains('<') && !inner.contains('\n') {
                    return true;
                }
            }
        }
        i += 1;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_basico() {
        assert_eq!(slugify("Revisión de PRs"), "revision-de-prs");
        assert_eq!(slugify("  --Hola   Mundo!! "), "hola-mundo");
        assert_eq!(slugify("año_2026"), "ano-2026");
    }

    #[test]
    fn fold_acentos() {
        assert_eq!(fold("ÁRBOL Ñandú"), "arbol nandu");
    }

    #[test]
    fn xml() {
        assert!(has_xml_tag("usa <tool> para"));
        assert!(has_xml_tag("cierra </x>"));
        assert!(!has_xml_tag("a < b y c > d"));
        assert!(!has_xml_tag("flecha -> y <= 3"));
    }

    #[test]
    fn distancia() {
        assert_eq!(edit_distance("pdf-tools", "pdf-tool"), 1);
        assert_eq!(edit_distance("", "abc"), 3);
    }
}
