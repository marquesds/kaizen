// SPDX-License-Identifier: AGPL-3.0-or-later
//! `kaizen insights` — workspace activity dashboard.

use crate::core::config;
use crate::core::data_source::DataSource;
use crate::metrics::report;
use crate::shell::cli::maybe_refresh_store;
use crate::shell::fmt::fmt_ts;
use crate::shell::remote_pull::maybe_telemetry_pull;
use crate::shell::scope;
use crate::store::InsightsStats;
use anyhow::Result;
use std::collections::HashMap;
use std::fmt::Write;
use std::path::Path;

/// Same output as `kaizen insights` stdout.
pub fn insights_text(
    workspace: Option<&Path>,
    all_workspaces: bool,
    refresh: bool,
    source: DataSource,
) -> Result<String> {
    let roots = scope::resolve(workspace, all_workspaces)?;
    let mut stats_rows = Vec::new();
    let mut reports = Vec::new();
    let mut guidance = String::new();
    for workspace in &roots {
        let cfg = config::load(workspace)?;
        let store = crate::store::Store::open(&crate::core::workspace::db_path(workspace))?;
        maybe_telemetry_pull(workspace, &store, &cfg, source, refresh)?;
        maybe_refresh_store(workspace, &store, refresh)?;
        let ws_str = workspace.to_string_lossy().to_string();
        let row = store.insights(&ws_str)?;
        let row = if source != DataSource::Local
            && let Ok(Some(agg)) =
                crate::shell::remote_observe::try_remote_event_agg(&store, &cfg, workspace)
        {
            crate::shell::remote_observe::merge_insights_stats(row, &agg, source)
        } else {
            row
        };
        stats_rows.push(row);
        if let Ok(report) = report::build_report(&store, &ws_str, 7) {
            reports.push(if roots.len() == 1 {
                report
            } else {
                decorate_metrics(workspace, report)
            });
        }
        if roots.len() == 1 {
            guidance =
                crate::shell::guidance::format_guidance_teaser(&store, workspace, &ws_str, 7)
                    .unwrap_or_else(|_| String::new());
        }
    }
    let stats = merge_insights(stats_rows);
    let metrics = merge_metrics(reports);
    Ok(format_dashboard(
        &scope::label(&roots),
        &stats,
        metrics.as_ref(),
        &guidance,
    ))
}

/// Print workspace activity dashboard.
pub fn cmd_insights(
    workspace: Option<&Path>,
    all_workspaces: bool,
    refresh: bool,
    source: DataSource,
) -> Result<()> {
    print!(
        "{}",
        insights_text(workspace, all_workspaces, refresh, source)?
    );
    Ok(())
}

fn merge_insights(rows: Vec<InsightsStats>) -> InsightsStats {
    let mut sessions_by_day = HashMap::new();
    let mut recent = Vec::new();
    let mut top_tools = HashMap::new();
    let mut total_sessions = 0;
    let mut running_sessions = 0;
    let mut total_events = 0;
    let mut total_cost_usd_e6 = 0;
    let mut sessions_with_cost = 0;
    for row in rows {
        total_sessions += row.total_sessions;
        running_sessions += row.running_sessions;
        total_events += row.total_events;
        total_cost_usd_e6 += row.total_cost_usd_e6;
        sessions_with_cost += row.sessions_with_cost;
        for (day, count) in row.sessions_by_day {
            *sessions_by_day.entry(day).or_insert(0_u64) += count;
        }
        recent.extend(row.recent);
        for (tool, count) in row.top_tools {
            *top_tools.entry(tool).or_insert(0_u64) += count;
        }
    }
    recent.sort_by(|a, b| {
        b.0.started_at_ms
            .cmp(&a.0.started_at_ms)
            .then_with(|| a.0.id.cmp(&b.0.id))
    });
    recent.truncate(3);
    let mut sessions_by_day = sessions_by_day.into_iter().collect::<Vec<_>>();
    sessions_by_day.sort_by(|a, b| a.0.cmp(&b.0));
    let mut top_tools = top_tools.into_iter().collect::<Vec<_>>();
    top_tools.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    top_tools.truncate(5);
    InsightsStats {
        total_sessions,
        running_sessions,
        total_events,
        sessions_by_day,
        recent,
        top_tools,
        total_cost_usd_e6,
        sessions_with_cost,
    }
}

fn decorate_metrics(
    workspace: &Path,
    mut metrics: crate::metrics::types::MetricsReport,
) -> crate::metrics::types::MetricsReport {
    for row in &mut metrics.hottest_files {
        row.path = scope::decorate_path(workspace, &row.path);
    }
    for row in &mut metrics.most_changed_files {
        row.path = scope::decorate_path(workspace, &row.path);
    }
    for row in &mut metrics.most_complex_files {
        row.path = scope::decorate_path(workspace, &row.path);
    }
    for row in &mut metrics.highest_risk_files {
        row.path = scope::decorate_path(workspace, &row.path);
    }
    for row in &mut metrics.agent_pain_hotspots {
        row.path = scope::decorate_path(workspace, &row.path);
    }
    metrics
}

fn merge_metrics(
    rows: Vec<crate::metrics::types::MetricsReport>,
) -> Option<crate::metrics::types::MetricsReport> {
    let mut it = rows.into_iter();
    let first = it.next()?;
    let mut out = crate::metrics::types::MetricsReport {
        snapshot: None,
        hottest_files: first.hottest_files,
        most_changed_files: first.most_changed_files,
        most_complex_files: first.most_complex_files,
        highest_risk_files: first.highest_risk_files,
        slowest_tools: first.slowest_tools,
        highest_token_tools: first.highest_token_tools,
        highest_reasoning_tools: first.highest_reasoning_tools,
        agent_pain_hotspots: first.agent_pain_hotspots,
    };
    for row in it {
        out.hottest_files.extend(row.hottest_files);
        out.most_changed_files.extend(row.most_changed_files);
        out.most_complex_files.extend(row.most_complex_files);
        out.highest_risk_files.extend(row.highest_risk_files);
        out.agent_pain_hotspots.extend(row.agent_pain_hotspots);
        merge_tool_rows(&mut out.slowest_tools, row.slowest_tools);
        merge_tool_rows(&mut out.highest_token_tools, row.highest_token_tools);
        merge_tool_rows(
            &mut out.highest_reasoning_tools,
            row.highest_reasoning_tools,
        );
    }
    trim_file_rows(&mut out.hottest_files);
    trim_file_rows(&mut out.most_changed_files);
    trim_file_rows(&mut out.most_complex_files);
    trim_file_rows(&mut out.highest_risk_files);
    trim_file_rows(&mut out.agent_pain_hotspots);
    trim_tool_rows(&mut out.slowest_tools, |row| row.p95_ms.unwrap_or(0));
    trim_tool_rows(&mut out.highest_token_tools, |row| row.total_tokens);
    trim_tool_rows(&mut out.highest_reasoning_tools, |row| {
        row.total_reasoning_tokens
    });
    Some(out)
}

fn merge_tool_rows(
    target: &mut Vec<crate::metrics::types::RankedTool>,
    rows: Vec<crate::metrics::types::RankedTool>,
) {
    for row in rows {
        if let Some(existing) = target.iter_mut().find(|item| item.tool == row.tool) {
            existing.calls += row.calls;
            existing.total_tokens += row.total_tokens;
            existing.total_reasoning_tokens += row.total_reasoning_tokens;
            existing.p50_ms = existing.p50_ms.max(row.p50_ms);
            existing.p95_ms = existing.p95_ms.max(row.p95_ms);
            continue;
        }
        target.push(row);
    }
}

fn trim_file_rows(rows: &mut Vec<crate::metrics::types::RankedFile>) {
    rows.sort_by(|a, b| b.value.cmp(&a.value).then_with(|| a.path.cmp(&b.path)));
    rows.truncate(10);
}

fn trim_tool_rows<F>(rows: &mut Vec<crate::metrics::types::RankedTool>, rank: F)
where
    F: Fn(&crate::metrics::types::RankedTool) -> u64,
{
    rows.sort_by(|a, b| rank(b).cmp(&rank(a)).then_with(|| a.tool.cmp(&b.tool)));
    rows.truncate(10);
}

fn format_dashboard(
    ws: &str,
    stats: &InsightsStats,
    metrics: Option<&crate::metrics::types::MetricsReport>,
    guidance_teaser: &str,
) -> String {
    let mut s = String::new();
    writeln!(&mut s, "kaizen — {ws}").unwrap();
    writeln!(&mut s).unwrap();
    format_sessions(&mut s, stats);
    writeln!(&mut s).unwrap();
    format_tools(&mut s, stats);
    writeln!(&mut s).unwrap();
    format_cost(&mut s, stats);
    if !guidance_teaser.is_empty() {
        writeln!(&mut s).unwrap();
        s.push_str(guidance_teaser);
    }
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
