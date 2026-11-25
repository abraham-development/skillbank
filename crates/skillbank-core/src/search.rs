//! Búsqueda en memoria (sin acentos ni mayúsculas) por nombre, etiquetas, descripción y contenido.

use crate::bank::Bank;
use crate::error::Result;
use crate::skill::SkillDoc;
use crate::text::fold;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct SearchHit {
    pub name: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub score: u32,
    /// name | tags | description | body
    pub matched_in: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
}

fn snippet_for(text: &str, term: &str) -> Option<String> {
    for line in text.lines() {
        if fold(line).contains(term) {
            let t = line.trim();
            let s: String = t.chars().take(160).collect();
            return Some(if t.chars().count() > 160 { format!("{s}…") } else { s });
        }
    }
    None
}

/// Busca `query` (todas las palabras deben aparecer). `tag` filtra por etiqueta.
pub fn search(bank: &Bank, query: &str, tag: Option<&str>, limit: usize) -> Result<Vec<SearchHit>> {
    let terms: Vec<String> = fold(query).split_whitespace().map(str::to_string).collect();
    let tag = tag.map(fold);
    let mut hits = Vec::new();
    for name in bank.names()? {
        let meta = bank.meta(&name);
        if let Some(t) = &tag
            && !meta.tags.iter().any(|x| fold(x) == *t)
        {
            continue;
        }
        let Ok(doc) = SkillDoc::load(&bank.skill_dir(&name)) else { continue };
        let fname = fold(&name);
        let fdesc = fold(doc.description.as_deref().unwrap_or(""));
        let fbody = fold(&doc.body);
        let ftags: Vec<String> = meta.tags.iter().map(|t| fold(t)).collect();

        let mut score = 0u32;
        let mut matched = Vec::new();
        let mut all_terms = true;
        let mut snippet = None;
        for term in &terms {
            let mut s = 0u32;
            if fname == *term {
                s += 100;
                matched.push("name");
            } else if fname.contains(term.as_str()) {
                s += 50;
                matched.push("name");
            }
            if ftags.iter().any(|t| t == term) {
                s += 40;
                matched.push("tags");
            } else if ftags.iter().any(|t| t.contains(term.as_str())) {
                s += 20;
                matched.push("tags");
            }
            if fdesc.contains(term.as_str()) {
                s += 20;
                matched.push("description");
                if snippet.is_none() {
                    snippet = doc.description.as_deref().and_then(|d| snippet_for(d, term));
                }
            }
            let body_hits = fbody.matches(term.as_str()).count() as u32;
            if body_hits > 0 {
                s += 5 * body_hits.min(5);
                matched.push("body");
                if snippet.is_none() {
                    snippet = snippet_for(&doc.body, term);
                }
            }
            if s == 0 {
                all_terms = false;
                break;
            }
            score += s;
        }
        if terms.is_empty() {
            score = 1;
        }
        if !all_terms || score == 0 {
            continue;
        }
        matched.sort();
        matched.dedup();
        hits.push(SearchHit {
            name: name.clone(),
            description: doc.description.clone(),
            tags: meta.tags.clone(),
            score,
            matched_in: matched.into_iter().map(str::to_string).collect(),
            snippet,
        });
    }
    hits.sort_by(|a, b| b.score.cmp(&a.score).then(a.name.cmp(&b.name)));
    hits.truncate(limit);
    Ok(hits)
}
