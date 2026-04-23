// SPDX-License-Identifier: AGPL-3.0-or-later
//! `kaizen insights` — workspace activity dashboard.

use crate::core::config;
use crate::metrics::{index, report};
use crate::shell::cli::{scan_all_agents, workspace_path};
use crate::shell::fmt::fmt_ts;
use crate::store::InsightsStats;
use crate::store::Store;
use anyhow::Result;
use std::path::Path;

/// Print workspace activity dashboard.
pub fn cmd_insights(workspace: Option<&Path>) -> Result<()> {
    let ws = workspace_path(workspace)?;
    let cfg = config::load(&ws)?;
    let ws_str = ws.to_string_lossy().to_string();
    let db_path = ws.join(".kaizen/kaizen.db");
    let store = Store::open(&db_path)?;
    scan_all_agents(&ws, &cfg, &ws_str, &store)?;
    let stats = store.insights(&ws_str)?;
    let metrics = index::ensure_indexed(&store, &ws, false)
        .ok()
        .and_then(|_| report::build_report(&store, &ws_str, 7).ok());
    if let Some(ctx) = crate::sync::ingest_ctx(&cfg, ws.clone())
        && let Some(snapshot) = metrics.as_ref().and_then(|report| report.snapshot.as_ref())
        && let Ok(facts) = store.file_facts_for_snapshot(&snapshot.id)
        && let Ok(edges) = store.repo_edges_for_snapshot(&snapshot.id)
    {
        let _ = crate::sync::smart::enqueue_repo_snapshot(&store, snapshot, &facts, &edges, &ctx);
    }
    print_dashboard(&ws_str, &stats, metrics.as_ref());
    Ok(())
}

fn print_dashboard(
    ws: &str,
    stats: &InsightsStats,
    metrics: Option<&crate::metrics::types::MetricsReport>,
) {
    println!("kaizen — {ws}");
    println!();
    print_sessions(stats);
    println!();
    print_tools(stats);
    println!();
    print_cost(stats);
    if let Some(metrics) = metrics {
        println!();
        print_code(metrics);
        println!();
        print_tool_spans(metrics);
    }
}

fn print_sessions(stats: &InsightsStats) {
    println!(
        "Sessions ({} total, {} running)",
        stats.total_sessions, stats.running_sessions
    );
    let day_parts: Vec<String> = stats
        .sessions_by_day
        .iter()
        .map(|(d, c)| format!("{d} {c}"))
        .collect();
    println!("  Last 7 days:  {}", day_parts.join("  "));
    if stats.recent.is_empty() {
        return;
    }
    println!("  Most recent:");
    for (s, cnt) in &stats.recent {
        println!(
            "    {}  {:<8}  {:<8}  {} events",
            fmt_ts(s.started_at_ms),
            s.agent,
            format!("{:?}", s.status),
            cnt
        );
    }
}

fn print_tools(stats: &InsightsStats) {
    println!("Tools (top 5)");
    let max = stats.top_tools.first().map(|(_, c)| *c).unwrap_or(1).max(1);
    for (tool, cnt) in &stats.top_tools {
        let bar_len = (cnt * 20 / max).max(1) as usize;
        let bar = "█".repeat(bar_len);
        println!("  {:<14} {:>5}  {}", tool, cnt, bar);
    }
    if stats.top_tools.is_empty() {
        println!("  (no tool data)");
    }
}

fn print_cost(stats: &InsightsStats) {
    let cost = stats.total_cost_usd_e6 as f64 / 1_000_000.0;
    println!(
        "Cost:  ${cost:.2}  ({} sessions with cost data)",
        stats.sessions_with_cost
    );
}

fn print_code(metrics: &crate::metrics::types::MetricsReport) {
    println!("Code");
    for row in metrics.hottest_files.iter().take(5) {
        println!("  hot {:>8}  {}", row.value, row.path);
    }
    for row in metrics.agent_pain_hotspots.iter().take(5) {
        println!("  pain {:>7}  {}", row.value, row.path);
    }
    if metrics.hottest_files.is_empty() && metrics.agent_pain_hotspots.is_empty() {
        println!("  (no file metrics)");
    }
}

fn print_tool_spans(metrics: &crate::metrics::types::MetricsReport) {
    println!("Tool Spans");
    for row in metrics.slowest_tools.iter().take(5) {
        let p95 = row
            .p95_ms
            .map(|v| format!("{v}ms"))
            .unwrap_or_else(|| "-".into());
        println!(
            "  {:<14} p95={} tok={} rtok={}",
            row.tool, p95, row.total_tokens, row.total_reasoning_tokens
        );
    }
    if metrics.slowest_tools.is_empty() {
        println!("  (no span metrics)");
    }
}
