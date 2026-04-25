// SPDX-License-Identifier: AGPL-3.0-or-later
//! Diff two `PromptSnapshot`s to find added, removed, and changed files.

use crate::prompt::types::{PromptDiff, PromptSnapshot};
use std::collections::HashMap;

/// Compute the diff between snapshot `a` (before) and `b` (after).
pub fn diff(a: &PromptSnapshot, b: &PromptSnapshot) -> PromptDiff {
    let files_a = a.files();
    let map_a: HashMap<&str, &str> = files_a
        .iter()
        .map(|f| (f.path.as_str(), f.sha256.as_str()))
        .collect();
    let map_b: HashMap<String, String> =
        b.files().into_iter().map(|f| (f.path, f.sha256)).collect();
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut changed = Vec::new();
    for (path, hash_b) in &map_b {
        match map_a.get(path.as_str()) {
            None => added.push(path.clone()),
            Some(hash_a) if *hash_a != hash_b.as_str() => changed.push(path.clone()),
            _ => {}
        }
    }
    for path in map_a.keys() {
        if !map_b.contains_key(*path) {
            removed.push((*path).to_string());
        }
    }
    added.sort();
    removed.sort();
    changed.sort();
    PromptDiff {
        added,
        removed,
        changed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompt::types::PromptFile;

    fn snap(files: &[(&str, &str)]) -> PromptSnapshot {
        let pfs: Vec<PromptFile> = files
            .iter()
            .map(|(p, h)| PromptFile {
                path: p.to_string(),
                sha256: h.to_string(),
                bytes: 0,
            })
            .collect();
        PromptSnapshot {
            fingerprint: "x".into(),
            captured_at_ms: 0,
            files_json: serde_json::to_string(&pfs).unwrap(),
            total_bytes: 0,
        }
    }

    #[test]
    fn identical_snapshots_empty_diff() {
        let a = snap(&[("a.md", "h1")]);
        let d = diff(&a, &a);
        assert!(d.is_empty());
    }

    #[test]
    fn detects_added_removed_changed() {
        let a = snap(&[("a.md", "h1"), ("b.md", "h2")]);
        let b = snap(&[("a.md", "h9"), ("c.md", "h3")]);
        let d = diff(&a, &b);
        assert_eq!(d.changed, vec!["a.md"]);
        assert_eq!(d.added, vec!["c.md"]);
        assert_eq!(d.removed, vec!["b.md"]);
    }
}
