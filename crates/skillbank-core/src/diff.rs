//! Diferencias entre dos versiones de una skill (para pedir confirmación antes de sobrescribir).

use crate::error::Result;
use crate::fsutil::list_files;
use crate::skill::SkillDoc;
use serde::Serialize;
use similar::{ChangeTag, TextDiff};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct FileChange {
    pub path: String,
    /// added | removed | modified
    pub status: String,
    pub added: usize,
    pub removed: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct KeyChange {
    pub key: String,
    /// added | removed | modified
    pub change: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct TreeDiff {
    pub files: Vec<FileChange>,
    pub frontmatter: Vec<KeyChange>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill_md_patch: Option<String>,
    pub patch_truncated: bool,
}

impl TreeDiff {
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Resumen corto en español, p. ej. "SKILL.md modificado (+12 −3), scripts/x.py añadido".
    pub fn summary_es(&self) -> String {
        self.files
            .iter()
            .map(|f| {
                let st = match f.status.as_str() {
                    "added" => "añadido",
                    "removed" => "eliminado",
                    _ => "modificado",
                };
                if f.added + f.removed > 0 {
                    format!("{} {st} (+{} −{})", f.path, f.added, f.removed)
                } else {
                    format!("{} {st}", f.path)
                }
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn line_counts(old: &str, new: &str) -> (usize, usize) {
    let d = TextDiff::from_lines(old, new);
    let mut added = 0;
    let mut removed = 0;
    for ch in d.iter_all_changes() {
        match ch.tag() {
            ChangeTag::Insert => added += 1,
            ChangeTag::Delete => removed += 1,
            ChangeTag::Equal => {}
        }
    }
    (added, removed)
}

/// Compara `old` (p. ej. el banco) con `new` (p. ej. la copia local).
/// `max_patch_lines`: recorta el parche de SKILL.md (None = completo).
pub fn diff_trees(
    old: &Path,
    new: &Path,
    old_label: &str,
    new_label: &str,
    max_patch_lines: Option<usize>,
) -> Result<TreeDiff> {
    let old_files: BTreeMap<String, _> = list_files(old)?.into_iter().map(|f| (f.rel.clone(), f)).collect();
    let new_files: BTreeMap<String, _> = list_files(new)?.into_iter().map(|f| (f.rel.clone(), f)).collect();
    let mut out = TreeDiff::default();

    let mut all: Vec<&String> = old_files.keys().chain(new_files.keys()).collect();
    all.sort();
    all.dedup();
    for rel in all {
        match (old_files.get(rel), new_files.get(rel)) {
            (None, Some(n)) => {
                let added = std::fs::read_to_string(&n.abs).map(|t| t.lines().count()).unwrap_or(0);
                out.files.push(FileChange { path: rel.clone(), status: "added".into(), added, removed: 0 });
            }
            (Some(o), None) => {
                let removed = std::fs::read_to_string(&o.abs).map(|t| t.lines().count()).unwrap_or(0);
                out.files.push(FileChange { path: rel.clone(), status: "removed".into(), added: 0, removed });
            }
            (Some(o), Some(n)) => {
                let ob = std::fs::read(&o.abs)?;
                let nb = std::fs::read(&n.abs)?;
                if ob == nb {
                    continue;
                }
                let (added, removed) = match (std::str::from_utf8(&ob), std::str::from_utf8(&nb)) {
                    (Ok(a), Ok(b)) => line_counts(a, b),
                    _ => (0, 0),
                };
                out.files.push(FileChange { path: rel.clone(), status: "modified".into(), added, removed });
            }
            (None, None) => {}
        }
    }

    // Claves del frontmatter
    let od = SkillDoc::load(old).ok();
    let nd = SkillDoc::load(new).ok();
    if let (Some(od), Some(nd)) = (&od, &nd) {
        let om = od.frontmatter.as_object().cloned().unwrap_or_default();
        let nm = nd.frontmatter.as_object().cloned().unwrap_or_default();
        let mut keys: Vec<&String> = om.keys().chain(nm.keys()).collect();
        keys.sort();
        keys.dedup();
        for k in keys {
            let change = match (om.get(k), nm.get(k)) {
                (None, Some(_)) => "added",
                (Some(_), None) => "removed",
                (Some(a), Some(b)) if a != b => "modified",
                _ => continue,
            };
            out.frontmatter.push(KeyChange { key: k.clone(), change: change.into() });
        }
        if od.raw != nd.raw {
            let d = TextDiff::from_lines(&od.raw, &nd.raw);
            let patch = d
                .unified_diff()
                .context_radius(3)
                .header(&format!("{old_label}/SKILL.md"), &format!("{new_label}/SKILL.md"))
                .to_string();
            let lines: Vec<&str> = patch.lines().collect();
            match max_patch_lines {
                Some(max) if lines.len() > max => {
                    out.skill_md_patch = Some(lines[..max].join("\n"));
                    out.patch_truncated = true;
                }
                _ => out.skill_md_patch = Some(patch),
            }
        }
    }
    Ok(out)
}
