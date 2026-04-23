// SPDX-License-Identifier: AGPL-3.0-or-later
//! `kaizen insights` — workspace activity dashboard.

use crate::core::config;
use crate::metrics::{index, report};
use crate::shell::cli::{maybe_scan_all_agents, workspace_path};
use crate::shell::fmt::fmt_ts;
use crate::store::InsightsStats;
use crate::store::Store;
use anyhow::Result;
use std::fmt::Write;
use std::path::Path;

/// Same output as `kaizen insights` stdout.
pub fn insights_text(workspace: Option<&Path>, refresh: bool) -> Result<String> {
    let ws = workspace_path(workspace)?;
    let cfg = config::load(&ws)?;
    let ws_str = ws.to_string_lossy().to_string();
    let db_path = ws.join(".kaizen/kaizen.db");
    let store = Store::open(&db_path)?;
    maybe_scan_all_agents(&ws, &cfg, &ws_str, &store, refresh)?;
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
    Ok(format_dashboard(&ws_str, &stats, metrics.as_ref()))
}

/// Print workspace activity dashboard.
pub fn cmd_insights(workspace: Option<&Path>, refresh: bool) -> Result<()> {
    print!("{}", insights_text(workspace, refresh)?);
    Ok(())
}

fn format_dashboard(
    ws: &str,
    stats: &InsightsStats,
    metrics: Option<&crate::metrics::types::MetricsReport>,
) -> String {
    let mut s = String::new();
    writeln!(&mut s, "kaizen — {ws}").unwrap();
    writeln!(&mut s).unwrap();
    format_sessions(&mut s, stats);
    writeln!(&mut s).unwrap();
    format_tools(&mut s, stats);
    writeln!(&mut s).unwrap();
    format_cost(&mut s, stats);
    if let Some(metrics) = metrics {
        writeln!(&mut s).unwrap();
        format_code(&mut s, metrics);
        writeln!(&mut s).unwrap();
        format_tool_spans(&mut s, metrics);
    }
    writeln!(&mut s).unwrap();
    s.push_str(&takeaway_block(ws, stats, metrics));
    s
}

fn takeaway_block(
    _ws: &str,
    stats: &InsightsStats,
    metrics: Option<&crate::metrics::types::MetricsReport>,
) -> String {
    use std::fmt::Write;
    let mut s = String::new();
    let _ = writeln!(&mut s, "Takeaway");
    if let Some(m) = metrics {
        if let Some(f) = m.hottest_files.first() {
            let _ = writeln!(
                &mut s,
                "  · Hottest file (agent × churn signal): {} — value {}",
                f.path, f.value
            );
        }
        if let Some(t) = m.slowest_tools.first() {
            let p95 = t
                .p95_ms
                .map(|v| format!("{v}ms"))
                .unwrap_or_else(|| "n/a".into());
            let _ = writeln!(&mut s, "  · Slowest tool (p95): {} @ {}", t.tool, p95);
        }
    }
    if let Some((rec, _n)) = stats.recent.first() {
        let _ = writeln!(&mut s, "  · Recent session agent: {}", rec.agent);
    }
    if !stats.top_tools.is_empty() {
        let _ = writeln!(
            &mut s,
            "  · Next: `kaizen retro --days 7` for ranked bets, or `kaizen exp new` to A/B a change"
        );
    } else {
        let _ = writeln!(
            &mut s,
            "  · Next: `kaizen metrics` or run more agent sessions to populate tools"
        );
    }
    s
}

fn format_sessions(out: &mut String, stats: &InsightsStats) {
    let _ = writeln!(
        out,
        "Sessions ({} total, {} running)",
        stats.total_sessions, stats.running_sessions
    );
    let day_parts: Vec<String> = stats
        .sessions_by_day
        .iter()
        .map(|(d, c)| format!("{d} {c}"))
        .collect();
    let _ = writeln!(out, "  Last 7 days:  {}", day_parts.join("  "));
    if stats.recent.is_empty() {
        return;
    }
    let _ = writeln!(out, "  Most recent:");
    for (s, cnt) in &stats.recent {
        let _ = writeln!(
            out,
            "    {}  {:<8}  {:<8}  {} events",
            fmt_ts(s.started_at_ms),
            s.agent,
            format!("{:?}", s.status),
            cnt
        );
    }
}

fn format_tools(out: &mut String, stats: &InsightsStats) {
    let _ = writeln!(out, "Tools (top 5)");
    let max = stats.top_tools.first().map(|(_, c)| *c).unwrap_or(1).max(1);
    for (tool, cnt) in &stats.top_tools {
        let bar_len = (cnt * 20 / max).max(1) as usize;
        let bar = "█".repeat(bar_len);
        let _ = writeln!(out, "  {:<14} {:>5}  {}", tool, cnt, bar);
    }
    if stats.top_tools.is_empty() {
        let _ = writeln!(out, "  (no tool data)");
    }
}

fn format_cost(out: &mut String, stats: &InsightsStats) {
    let cost = stats.total_cost_usd_e6 as f64 / 1_000_000.0;
    let _ = writeln!(
        out,
        "Cost:  ${cost:.2}  ({} sessions with cost data)",
        stats.sessions_with_cost
    );
}

fn format_code(out: &mut String, metrics: &crate::metrics::types::MetricsReport) {
    let _ = writeln!(out, "Code");
    for row in metrics.hottest_files.iter().take(5) {
        let _ = writeln!(out, "  hot {:>8}  {}", row.value, row.path);
    }
    for row in metrics.agent_pain_hotspots.iter().take(5) {
        let _ = writeln!(out, "  pain {:>7}  {}", row.value, row.path);
    }
    if metrics.hottest_files.is_empty() && metrics.agent_pain_hotspots.is_empty() {
        let _ = writeln!(out, "  (no file metrics)");
    }
}

fn format_tool_spans(out: &mut String, metrics: &crate::metrics::types::MetricsReport) {
    let _ = writeln!(out, "Tool Spans");
    for row in metrics.slowest_tools.iter().take(5) {
        let p95 = row
            .p95_ms
            .map(|v| format!("{v}ms"))
            .unwrap_or_else(|| "-".into());
        let _ = writeln!(
            out,
            "  {:<14} p95={} tok={} rtok={}",
            row.tool, p95, row.total_tokens, row.total_reasoning_tokens
        );
    }
    if metrics.slowest_tools.is_empty() {
        let _ = writeln!(out, "  (no span metrics)");
    }
}
