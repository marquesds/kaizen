//! `kaizen insights` — workspace activity dashboard.

use crate::core::config;
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
    print_dashboard(&ws_str, &stats);
    Ok(())
}

fn print_dashboard(ws: &str, stats: &InsightsStats) {
    println!("kaizen — {ws}");
    println!();
    print_sessions(stats);
    println!();
    print_tools(stats);
    println!();
    print_cost(stats);
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
