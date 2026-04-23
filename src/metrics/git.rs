// SPDX-License-Identifier: AGPL-3.0-or-later
//! Git-derived churn, authorship, co-change.

use crate::metrics::types::{FileHistory, RepoEdge};
use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::process::Command;

pub fn load_history(
    workspace: &Path,
    now_ms: u64,
) -> Result<(HashMap<String, FileHistory>, Vec<RepoEdge>)> {
    if !workspace.join(".git").exists() {
        return Ok((HashMap::new(), vec![]));
    }
    let out = Command::new("git")
        .arg("-C")
        .arg(workspace)
        .args([
            "log",
            "--since=90 days ago",
            "--format=__K__%H|%ct|%ae",
            "--name-only",
        ])
        .output()
        .context("git log")?;
    if !out.status.success() {
        anyhow::bail!("git log failed: {}", String::from_utf8_lossy(&out.stderr));
    }
    Ok(parse_history(&String::from_utf8_lossy(&out.stdout), now_ms))
}

fn parse_history(raw: &str, now_ms: u64) -> (HashMap<String, FileHistory>, Vec<RepoEdge>) {
    let mut histories: HashMap<String, FileHistory> = HashMap::new();
    let mut authors: HashMap<String, HashSet<String>> = HashMap::new();
    let mut co_changed: HashMap<(String, String), u32> = HashMap::new();
    let mut current_ts = 0u64;
    let mut current_author = String::new();
    let mut current_paths: Vec<String> = vec![];

    for line in raw.lines().chain(std::iter::once("__END__")) {
        if let Some(meta) = line.strip_prefix("__K__") {
            flush_commit(&current_paths, &mut co_changed);
            current_paths.clear();
            let mut parts = meta.split('|');
            let _hash = parts.next();
            current_ts = parts
                .next()
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0)
                * 1000;
            current_author = parts.next().unwrap_or("").to_string();
            continue;
        }
        let path = line.trim();
        if path.is_empty() || path == "__END__" {
            continue;
        }
        current_paths.push(path.to_string());
        let entry = histories.entry(path.to_string()).or_default();
        entry.churn_90d += 1;
        if current_ts >= now_ms.saturating_sub(30 * 86_400_000) {
            entry.churn_30d += 1;
        }
        if entry.last_changed_ms.is_none() {
            entry.last_changed_ms = Some(current_ts);
        }
        authors
            .entry(path.to_string())
            .or_default()
            .insert(current_author.clone());
    }

    for (path, set) in authors {
        histories.entry(path).or_default().authors_90d = set.len() as u32;
    }

    let edges = co_changed
        .into_iter()
        .map(|((from_path, to_path), weight)| RepoEdge {
            from_path,
            to_path,
            kind: "CO_CHANGED_WITH".into(),
            weight,
        })
        .collect();
    (histories, edges)
}

fn flush_commit(paths: &[String], out: &mut HashMap<(String, String), u32>) {
    for i in 0..paths.len() {
        for j in (i + 1)..paths.len() {
            let mut a = paths[i].clone();
            let mut b = paths[j].clone();
            if a > b {
                std::mem::swap(&mut a, &mut b);
            }
            *out.entry((a, b)).or_default() += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_log_history() {
        let raw = "__K__a|100|a@x\nsrc/a.rs\nsrc/b.rs\n__K__b|200|b@x\nsrc/a.rs\n";
        let (history, edges) = parse_history(raw, 250_000);
        assert_eq!(history["src/a.rs"].churn_90d, 2);
        assert_eq!(history["src/a.rs"].authors_90d, 2);
        assert_eq!(edges[0].weight, 1);
    }
}
