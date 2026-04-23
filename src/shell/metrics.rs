// SPDX-License-Identifier: AGPL-3.0-or-later
//! `kaizen metrics` command.

use crate::core::config;
use crate::metrics::{index, report};
use crate::shell::cli::{scan_all_agents, workspace_path};
use crate::store::Store;
use crate::sync::{ingest_ctx, smart};
use anyhow::Result;
use std::path::Path;

/// Same output as `kaizen metrics` (human or pretty JSON when `json_out`).
pub fn metrics_text(
    workspace: Option<&Path>,
    days: u32,
    json_out: bool,
    force: bool,
) -> Result<String> {
    let ws = workspace_path(workspace)?;
    let cfg = config::load(&ws)?;
    let db_path = ws.join(".kaizen/kaizen.db");
    let store = Store::open(&db_path)?;
    let ws_str = ws.to_string_lossy().to_string();
    scan_all_agents(&ws, &cfg, &ws_str, &store)?;
    let snapshot = index::ensure_indexed(&store, &ws, force)?;
    maybe_enqueue_snapshot(&store, &cfg, &ws, &snapshot)?;
    let metrics = report::build_report(&store, &ws_str, days)?;
    if json_out {
        return Ok(serde_json::to_string_pretty(&metrics)?);
    }
    Ok(format_human(&metrics))
}

pub fn cmd_metrics(workspace: Option<&Path>, days: u32, json_out: bool, force: bool) -> Result<()> {
    print!("{}", metrics_text(workspace, days, json_out, force)?);
    Ok(())
}

/// Same output as `kaizen metrics index`.
pub fn metrics_index_text(workspace: Option<&Path>, force: bool) -> Result<String> {
    let ws = workspace_path(workspace)?;
    let cfg = config::load(&ws)?;
    let db_path = ws.join(".kaizen/kaizen.db");
    let store = Store::open(&db_path)?;
    let snapshot = index::ensure_indexed(&store, &ws, force)?;
    maybe_enqueue_snapshot(&store, &cfg, &ws, &snapshot)?;
    use std::fmt::Write;
    let mut s = String::new();
    writeln!(&mut s, "snapshot: {}", snapshot.id).unwrap();
    writeln!(&mut s, "graph:    {}", snapshot.graph_path).unwrap();
    Ok(s)
}

pub fn cmd_metrics_index(workspace: Option<&Path>, force: bool) -> Result<()> {
    print!("{}", metrics_index_text(workspace, force)?);
    Ok(())
}

fn maybe_enqueue_snapshot(
    store: &Store,
    cfg: &crate::core::config::Config,
    ws: &std::path::Path,
    snapshot: &crate::metrics::types::RepoSnapshotRecord,
) -> Result<()> {
    let Some(ctx) = ingest_ctx(cfg, ws.to_path_buf()) else {
        return Ok(());
    };
    let facts = store.file_facts_for_snapshot(&snapshot.id)?;
    let edges = store.repo_edges_for_snapshot(&snapshot.id)?;
    smart::enqueue_repo_snapshot(store, snapshot, &facts, &edges, &ctx)
}

pub fn print_human(metrics: &crate::metrics::types::MetricsReport) {
    print!("{}", format_human(metrics));
}

fn format_files(title: &str, rows: &[crate::metrics::types::RankedFile]) -> String {
    use std::fmt::Write;
    let mut s = String::new();
    writeln!(&mut s, "{title}").unwrap();
    if rows.is_empty() {
        writeln!(&mut s, "  (none)").unwrap();
        writeln!(&mut s).unwrap();
        return s;
    }
    for row in rows.iter().take(5) {
        writeln!(&mut s, "  {:>8}  {}", row.value, row.path).unwrap();
    }
    writeln!(&mut s).unwrap();
    s
}

fn format_tools(title: &str, rows: &[crate::metrics::types::RankedTool]) -> String {
    use std::fmt::Write;
    let mut s = String::new();
    writeln!(&mut s, "{title}").unwrap();
    if rows.is_empty() {
        writeln!(&mut s, "  (none)").unwrap();
        writeln!(&mut s).unwrap();
        return s;
    }
    for row in rows.iter().take(5) {
        let p95 = row
            .p95_ms
            .map(|v| format!("{v}ms"))
            .unwrap_or_else(|| "-".into());
        writeln!(
            &mut s,
            "  {:<14} calls={} p95={} tok={} rtok={}",
            row.tool, row.calls, p95, row.total_tokens, row.total_reasoning_tokens
        )
        .unwrap();
    }
    writeln!(&mut s).unwrap();
    s
}

fn format_human(metrics: &crate::metrics::types::MetricsReport) -> String {
    let mut out = String::new();
    out.push_str(&format_files("Hottest files", &metrics.hottest_files));
    out.push_str(&format_files("Most changed", &metrics.most_changed_files));
    out.push_str(&format_files("Most complex", &metrics.most_complex_files));
    out.push_str(&format_files("Highest risk", &metrics.highest_risk_files));
    out.push_str(&format_tools("Slowest tools", &metrics.slowest_tools));
    out.push_str(&format_tools(
        "Highest token tools",
        &metrics.highest_token_tools,
    ));
    out.push_str(&format_tools(
        "Highest reasoning tools",
        &metrics.highest_reasoning_tools,
    ));
    out.push_str(&format_files(
        "Agent pain hotspots",
        &metrics.agent_pain_hotspots,
    ));
    out
}
