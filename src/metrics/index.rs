// SPDX-License-Identifier: AGPL-3.0-or-later
//! Build repo snapshot facts and sidecar.

use crate::core::repo::{binding_for_session, dirty_fingerprint, repo_head, tracked_files};
use crate::metrics::analyze::analyzer_for;
use crate::metrics::codegraph::{rebuild_sidecar, symbol_id};
use crate::metrics::git::load_history;
use crate::metrics::types::{FileFact, RepoAnalysis, RepoEdge, RepoSnapshotRecord, SymbolFact};
use crate::store::Store;
use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

const ANALYZER_VERSION: &str = "smart-metrics-v1";

pub fn ensure_indexed(store: &Store, workspace: &Path, force: bool) -> Result<RepoSnapshotRecord> {
    let now = now_ms();
    let workspace_str = workspace.to_string_lossy().to_string();
    let head_commit = repo_head(workspace)?;
    let dirty = binding_for_session(workspace, now, Some(now))
        .dirty_end
        .unwrap_or(false);
    let dirty_fp = dirty_fingerprint(workspace)?;
    let snapshot_id = snapshot_id(&workspace_str, head_commit.as_deref(), &dirty_fp);
    if !force
        && let Some(existing) = store.latest_repo_snapshot(&workspace_str)?
        && existing.id == snapshot_id
    {
        return Ok(existing);
    }

    let tracked = tracked_files(workspace)?;
    let (history, mut edges) = load_history(workspace, now)?;
    let mut analyses = tracked
        .iter()
        .filter_map(|rel| analyze_one(workspace, rel).ok())
        .collect::<Vec<_>>();
    let deps = resolve_dependencies(&analyses);
    edges.extend(deps.clone());
    let fan = fan_counts(&deps);
    let mut symbols = vec![];
    let graph_path = workspace.join(".kaizen/codegraph.db");
    let snapshot = RepoSnapshotRecord {
        id: snapshot_id,
        workspace: workspace_str.clone(),
        head_commit,
        dirty_fingerprint: dirty_fp,
        analyzer_version: ANALYZER_VERSION.into(),
        indexed_at_ms: now,
        dirty,
        graph_path: graph_path.to_string_lossy().to_string(),
    };
    let facts = analyses
        .iter_mut()
        .map(|analysis| {
            for symbol in &mut analysis.symbols {
                symbol.path = analysis.path.clone();
            }
            symbols.extend(analysis.symbols.clone());
            let history = history.get(&analysis.path).cloned().unwrap_or_default();
            let (fan_in, fan_out) = fan.get(&analysis.path).copied().unwrap_or((0, 0));
            FileFact {
                snapshot_id: snapshot.id.clone(),
                path: analysis.path.clone(),
                language: analysis.language.clone(),
                bytes: analysis.bytes,
                loc: analysis.loc,
                sloc: analysis.sloc,
                complexity_total: analysis.complexity_total,
                max_fn_complexity: analysis.max_fn_complexity,
                symbol_count: analysis.symbols.len() as u32,
                import_count: analysis.imports.len() as u32,
                fan_in,
                fan_out,
                churn_30d: history.churn_30d,
                churn_90d: history.churn_90d,
                authors_90d: history.authors_90d,
                last_changed_ms: history.last_changed_ms,
            }
        })
        .collect::<Vec<_>>();
    edges.extend(call_edges(&symbols));
    if let Some(parent) = graph_path.parent() {
        fs::create_dir_all(parent)?;
    }
    rebuild_sidecar(&graph_path, &snapshot, &facts, &symbols, &edges)?;
    store.save_repo_snapshot(&snapshot, &facts, &edges)?;
    Ok(snapshot)
}

fn analyze_one(workspace: &Path, rel: &str) -> Result<RepoAnalysis> {
    let full = workspace.join(rel);
    let source = fs::read_to_string(&full)?;
    analyzer_for(&full).analyze(rel, &source)
}

fn resolve_dependencies(analyses: &[RepoAnalysis]) -> Vec<RepoEdge> {
    let stem_index = analyses.iter().fold(
        HashMap::<String, Vec<String>>::new(),
        |mut acc, analysis| {
            let stem = Path::new(&analysis.path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            acc.entry(stem).or_default().push(analysis.path.clone());
            acc
        },
    );
    analyses
        .iter()
        .flat_map(|analysis| {
            analysis.imports.iter().filter_map(|raw| {
                resolve_import(&analysis.path, raw, &stem_index).map(|to_path| RepoEdge {
                    from_path: analysis.path.clone(),
                    to_path: to_path.clone(),
                    kind: "DEPENDS_ON".into(),
                    weight: 1,
                })
            })
        })
        .collect()
}

fn resolve_import(
    current_path: &str,
    raw: &str,
    stem_index: &HashMap<String, Vec<String>>,
) -> Option<String> {
    if raw.starts_with("./") || raw.starts_with("../") {
        return relative_target(current_path, raw);
    }
    let normalized = raw
        .trim_start_matches("crate::")
        .trim_start_matches("self::")
        .trim_start_matches("super::");
    let tail = normalized
        .split(['/', ':', '.'])
        .rfind(|part| !part.is_empty())
        .unwrap_or("");
    stem_index.get(tail).and_then(|paths| {
        if paths.len() == 1 {
            Some(paths[0].clone())
        } else {
            None
        }
    })
}

fn relative_target(current_path: &str, raw: &str) -> Option<String> {
    let base = Path::new(current_path).parent()?;
    let joined = base.join(raw);
    let candidates = [
        joined.clone(),
        joined.with_extension("rs"),
        joined.with_extension("ts"),
        joined.with_extension("js"),
        joined.with_extension("py"),
        joined.with_extension("go"),
        joined.with_extension("java"),
        joined.join("mod.rs"),
        joined.join("index.ts"),
        joined.join("index.js"),
    ];
    candidates.into_iter().find_map(|path| {
        path.to_str()
            .filter(|s| !s.is_empty())
            .map(|s| s.replace('\\', "/"))
    })
}

fn fan_counts(edges: &[RepoEdge]) -> HashMap<String, (u32, u32)> {
    let mut out = HashMap::new();
    for edge in edges.iter().filter(|edge| edge.kind == "DEPENDS_ON") {
        out.entry(edge.from_path.clone()).or_insert((0, 0)).1 += 1;
        out.entry(edge.to_path.clone()).or_insert((0, 0)).0 += 1;
    }
    out
}

fn call_edges(symbols: &[SymbolFact]) -> Vec<RepoEdge> {
    let mut by_name: HashMap<String, Vec<&SymbolFact>> = HashMap::new();
    for symbol in symbols {
        by_name.entry(symbol.name.clone()).or_default().push(symbol);
    }
    let mut out = vec![];
    for symbol in symbols {
        for call in &symbol.calls {
            let Some(targets) = by_name.get(call) else {
                continue;
            };
            if targets.len() != 1 {
                continue;
            }
            out.push(RepoEdge {
                from_path: symbol_id(symbol),
                to_path: symbol_id(targets[0]),
                kind: "CALLS".into(),
                weight: 1,
            });
        }
    }
    out
}

fn snapshot_id(workspace: &str, head: Option<&str>, dirty_fp: &str) -> String {
    crate::sync::hash_with_salt(
        &[7u8; 32],
        format!("{workspace}:{head:?}:{dirty_fp}:{ANALYZER_VERSION}").as_bytes(),
    )
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
