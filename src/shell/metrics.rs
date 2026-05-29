// SPDX-License-Identifier: AGPL-3.0-or-later
//! `kaizen metrics` command.

use crate::core::config;
use crate::core::data_source::DataSource;
use crate::metrics::{index, report};
use crate::shell::cli::{maybe_refresh_store, open_workspace_read_store, workspace_path};
use crate::shell::remote_pull::maybe_telemetry_pull;
use crate::shell::scope;
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
    all_workspaces: bool,
    refresh: bool,
    source: DataSource,
) -> Result<String> {
    let roots = scope::resolve(workspace, all_workspaces)?;
    let mut reports = Vec::new();
    for workspace in &roots {
        let cfg = config::load(workspace)?;
        let store = open_workspace_read_store(workspace, refresh || source != DataSource::Local)?;
        maybe_telemetry_pull(workspace, &store, &cfg, source, refresh)?;
        maybe_refresh_store(workspace, &store, refresh)?;
        if force {
            let snapshot = index::ensure_indexed(&store, workspace, true)?;
            maybe_enqueue_snapshot(&store, &cfg, workspace, &snapshot)?;
        }
        let ws_str = workspace.to_string_lossy().to_string();
        if let Ok(mut report) = report::build_report(&store, &ws_str, days) {
            if source != DataSource::Local
                && let Ok(Some(agg)) =
                    crate::shell::remote_observe::try_remote_event_agg(&store, &cfg, workspace)
            {
                report =
                    crate::shell::remote_observe::apply_remote_to_metrics(report, &agg, source);
            }
            reports.push(if roots.len() == 1 {
                report
            } else {
                decorate_metrics(workspace, report)
            });
        }
    }
    let metrics = merge_metrics(reports);
    if json_out {
        return Ok(serde_json::to_string_pretty(&metrics)?);
    }
    Ok(format_human(&metrics))
}

pub fn cmd_metrics(
    workspace: Option<&Path>,
    days: u32,
    json_out: bool,
    force: bool,
    all_workspaces: bool,
    refresh: bool,
    source: DataSource,
) -> Result<()> {
    print!(
        "{}",
        metrics_text(
            workspace,
            days,
            json_out,
            force,
            all_workspaces,
            refresh,
            source
        )?
    );
    Ok(())
}

/// Same output as `kaizen metrics index`.
pub fn metrics_index_text(workspace: Option<&Path>, force: bool) -> Result<String> {
    let ws = workspace_path(workspace)?;
    let cfg = config::load(&ws)?;
    let db_path = crate::core::workspace::db_path(&ws)?;
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

pub fn metrics_quality_text(workspace: Option<&Path>, days: u32, json: bool) -> Result<String> {
    let ws = workspace_path(workspace)?;
    let store = open_workspace_read_store(&ws, false)?;
    let end = now_ms();
    let start = end.saturating_sub(days as u64 * 86_400_000);
    let ws_str = ws.to_string_lossy().to_string();
    let report = crate::metrics::quality::build_quality_report(&store, &ws_str, start, end)?;
    if json {
        return Ok(format!("{}\n", serde_json::to_string_pretty(&report)?));
    }
    Ok(format_quality(&report))
}

pub fn cmd_metrics_quality(workspace: Option<&Path>, days: u32, json: bool) -> Result<()> {
    print!("{}", metrics_quality_text(workspace, days, json)?);
    Ok(())
}

fn format_quality(report: &crate::metrics::quality::CaptureQualityReport) -> String {
    format!(
        "Capture quality\n  events: {}\n  proxy_events: {}\n  trace_spans: {}\n  token_coverage: {}%\n  cost_coverage: {}%\n  latency_coverage: {}%\n  context_coverage: {}%\n  proxy_correlation: {}%\n  cache_read_tokens: {}\n  cache_creation_tokens: {}\n  orphan_spans: {}\n",
        report.events_total,
        report.proxy_events,
        report.trace_spans_total,
        report.token_coverage_pct,
        report.cost_coverage_pct,
        report.latency_coverage_pct,
        report.context_coverage_pct,
        report.proxy_correlation_pct,
        report.cache_read_tokens,
        report.cache_creation_tokens,
        report.orphan_span_count,
    )
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
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
) -> crate::metrics::types::MetricsReport {
    let mut out = crate::metrics::types::MetricsReport {
        snapshot: None,
        hottest_files: Vec::new(),
        most_changed_files: Vec::new(),
        most_complex_files: Vec::new(),
        highest_risk_files: Vec::new(),
        slowest_tools: Vec::new(),
        highest_token_tools: Vec::new(),
        highest_reasoning_tools: Vec::new(),
        agent_pain_hotspots: Vec::new(),
    };
    for row in rows {
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
    out
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
    smart::enqueue_repo_snapshot(store, snapshot, &facts, &edges, &ctx)?;
    smart::enqueue_workspace_fact_snapshot(store, ws, &ctx)
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
    use std::fmt::Write;
    let _ = writeln!(&mut out);
    let _ = writeln!(&mut out, "Takeaway");
    if let Some(f) = metrics.hottest_files.first() {
        let _ = writeln!(
            &mut out,
            "  · Review {} first (heat {}); pair with `kaizen insights` for session context",
            f.path, f.value
        );
    }
    if let Some(t) = metrics.slowest_tools.first() {
        let p95 = t
            .p95_ms
            .map(|v| format!("{v}ms"))
            .unwrap_or_else(|| "n/a".into());
        let _ = writeln!(
            &mut out,
            "  · Latency focus: {} p95 {} — tune or cache this tool path if it recurs",
            t.tool, p95
        );
    }
    if let Some(t) = metrics.highest_token_tools.first() {
        let _ = writeln!(
            &mut out,
            "  · Token sink: {} ({} total tok) — compare week over week with `kaizen metrics --days 30`",
            t.tool, t.total_tokens
        );
    }
    let _ = writeln!(
        &mut out,
        "  · Next: `kaizen retro --days 7` for team-level bets"
    );
    out
}
