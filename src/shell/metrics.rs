//! `kaizen metrics` command.

use crate::core::config;
use crate::metrics::{index, report};
use crate::shell::cli::{scan_all_agents, workspace_path};
use crate::store::Store;
use crate::sync::{ingest_ctx, smart};
use anyhow::Result;
use std::path::Path;

pub fn cmd_metrics(workspace: Option<&Path>, days: u32, json_out: bool, force: bool) -> Result<()> {
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
        println!("{}", serde_json::to_string_pretty(&metrics)?);
        return Ok(());
    }
    print_human(&metrics);
    Ok(())
}

pub fn cmd_metrics_index(workspace: Option<&Path>, force: bool) -> Result<()> {
    let ws = workspace_path(workspace)?;
    let cfg = config::load(&ws)?;
    let db_path = ws.join(".kaizen/kaizen.db");
    let store = Store::open(&db_path)?;
    let snapshot = index::ensure_indexed(&store, &ws, force)?;
    maybe_enqueue_snapshot(&store, &cfg, &ws, &snapshot)?;
    println!("snapshot: {}", snapshot.id);
    println!("graph:    {}", snapshot.graph_path);
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
    print_files("Hottest files", &metrics.hottest_files);
    print_files("Most changed", &metrics.most_changed_files);
    print_files("Most complex", &metrics.most_complex_files);
    print_files("Highest risk", &metrics.highest_risk_files);
    print_tools("Slowest tools", &metrics.slowest_tools);
    print_tools("Highest token tools", &metrics.highest_token_tools);
    print_tools("Highest reasoning tools", &metrics.highest_reasoning_tools);
    print_files("Agent pain hotspots", &metrics.agent_pain_hotspots);
}

fn print_files(title: &str, rows: &[crate::metrics::types::RankedFile]) {
    println!("{title}");
    if rows.is_empty() {
        println!("  (none)");
        println!();
        return;
    }
    for row in rows.iter().take(5) {
        println!("  {:>8}  {}", row.value, row.path);
    }
    println!();
}

fn print_tools(title: &str, rows: &[crate::metrics::types::RankedTool]) {
    println!("{title}");
    if rows.is_empty() {
        println!("  (none)");
        println!();
        return;
    }
    for row in rows.iter().take(5) {
        let p95 = row
            .p95_ms
            .map(|v| format!("{v}ms"))
            .unwrap_or_else(|| "-".into());
        println!(
            "  {:<14} calls={} p95={} tok={} rtok={}",
            row.tool, row.calls, p95, row.total_tokens, row.total_reasoning_tokens
        );
    }
    println!();
}
