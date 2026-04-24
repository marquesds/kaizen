// SPDX-License-Identifier: AGPL-3.0-or-later
//! Merge `remote_events` aggregates into `summary` / `insights` / `metrics` for non-local sources.

use crate::core::config::Config;
use crate::core::data_source::DataSource;
use crate::metrics::types::{MetricsReport, RankedTool};
use crate::store::{GuidanceReport, InsightsStats, RemoteEventAgg, Store, SummaryStats};
use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;

/// Load provider-side aggregates when `team_id` + `workspace_hash` (from config + workspace) are set.
/// Returns `Ok(None)` if sync context is missing; `Err` on DB errors.
pub fn try_remote_event_agg(
    store: &Store,
    cfg: &Config,
    workspace: &Path,
) -> Result<Option<RemoteEventAgg>> {
    if cfg.sync.team_id.trim().is_empty() {
        return Ok(None);
    }
    let Some(ctx) = crate::sync::ingest_ctx(cfg, workspace.to_path_buf()) else {
        return Ok(None);
    };
    let Some(wh) = crate::sync::smart::workspace_hash_for(&ctx) else {
        return Ok(None);
    };
    let agg = store.remote_event_aggregate(&cfg.sync.team_id, &wh)?;
    Ok(Some(agg))
}

fn merge_count_rows(rows: impl IntoIterator<Item = Vec<(String, u64)>>) -> Vec<(String, u64)> {
    let mut m: HashMap<String, u64> = HashMap::new();
    for set in rows {
        for (k, v) in set {
            *m.entry(k).or_insert(0) += v;
        }
    }
    let mut v: Vec<_> = m.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    v
}

fn summary_from_remote(r: &RemoteEventAgg) -> SummaryStats {
    SummaryStats {
        session_count: r.session_count,
        total_cost_usd_e6: r.total_cost_usd_e6,
        by_agent: r.by_agent.clone(),
        by_model: r.by_model.clone(),
        top_tools: r.top_tools.clone(),
    }
}

/// Merge or replace headline stats for `DataSource` (see `specs/observe-pipeline.qnt`).
pub fn merge_summary_stats(
    local: SummaryStats,
    remote: &RemoteEventAgg,
    source: DataSource,
) -> SummaryStats {
    if remote.event_count == 0 {
        return local;
    }
    match source {
        DataSource::Local => local,
        DataSource::Provider => summary_from_remote(remote),
        DataSource::Mixed => SummaryStats {
            session_count: local.session_count.saturating_add(remote.session_count),
            total_cost_usd_e6: local
                .total_cost_usd_e6
                .saturating_add(remote.total_cost_usd_e6),
            by_agent: merge_count_rows(vec![local.by_agent, remote.by_agent.clone()]),
            by_model: merge_count_rows(vec![local.by_model, remote.by_model.clone()]),
            top_tools: merge_count_rows(vec![local.top_tools, remote.top_tools.clone()]),
        },
    }
}

fn insights_from_remote(r: RemoteEventAgg) -> InsightsStats {
    let RemoteEventAgg {
        session_count,
        event_count,
        total_cost_usd_e6,
        sessions_with_cost,
        sessions_by_day,
        top_tools,
        ..
    } = r;
    let top_tools = top_tools.into_iter().take(5).collect();
    InsightsStats {
        total_sessions: session_count,
        running_sessions: 0,
        total_events: event_count,
        sessions_by_day,
        recent: vec![],
        top_tools,
        total_cost_usd_e6,
        sessions_with_cost,
    }
}

fn merge_insights_mixed(local: &InsightsStats, r: &RemoteEventAgg) -> InsightsStats {
    let mut out = local.clone();
    out.total_sessions = out.total_sessions.saturating_add(r.session_count);
    out.total_events = out.total_events.saturating_add(r.event_count);
    out.total_cost_usd_e6 = out.total_cost_usd_e6.saturating_add(r.total_cost_usd_e6);
    out.sessions_with_cost = out.sessions_with_cost.saturating_add(r.sessions_with_cost);
    if out.sessions_by_day.len() == r.sessions_by_day.len() {
        for (i, (_, c)) in r.sessions_by_day.iter().enumerate() {
            out.sessions_by_day[i].1 = out.sessions_by_day[i].1.saturating_add(*c);
        }
    }
    out.top_tools = merge_count_rows(vec![
        local.top_tools.clone(),
        r.top_tools.iter().take(10).cloned().collect(),
    ])
    .into_iter()
    .take(5)
    .collect();
    out
}

pub fn merge_insights_stats(
    local: InsightsStats,
    remote: &RemoteEventAgg,
    source: DataSource,
) -> InsightsStats {
    if remote.event_count == 0 {
        return local;
    }
    match source {
        DataSource::Local => local,
        DataSource::Provider => insights_from_remote(remote.clone()),
        DataSource::Mixed => merge_insights_mixed(&local, remote),
    }
}

/// Session count in guidance teaser: for `mixed`, add remote session cardinality; for `provider`, prefer the larger of local vs remote when remote is loaded.
pub fn merge_guidance_sessions_in_window(
    mut report: GuidanceReport,
    remote: &RemoteEventAgg,
    source: DataSource,
) -> GuidanceReport {
    if remote.event_count == 0 {
        return report;
    }
    match source {
        DataSource::Local => {}
        DataSource::Provider => {
            report.sessions_in_window = report.sessions_in_window.max(remote.session_count);
        }
        DataSource::Mixed => {
            report.sessions_in_window = report
                .sessions_in_window
                .saturating_add(remote.session_count);
        }
    }
    report
}

fn merge_one_tool_row(target: &mut Vec<RankedTool>, row: RankedTool) {
    if let Some(existing) = target.iter_mut().find(|t| t.tool == row.tool) {
        existing.calls = existing.calls.saturating_add(row.calls);
        existing.total_tokens = existing.total_tokens.saturating_add(row.total_tokens);
        existing.total_reasoning_tokens = existing
            .total_reasoning_tokens
            .saturating_add(row.total_reasoning_tokens);
        existing.p50_ms = match (existing.p50_ms, row.p50_ms) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (a, b) => a.or(b),
        };
        existing.p95_ms = match (existing.p95_ms, row.p95_ms) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (a, b) => a.or(b),
        };
        return;
    }
    target.push(row);
}

/// Fold remote event-derived tool call / token stats into the tool ranking sections only (files stay local).
pub fn apply_remote_to_metrics(
    mut report: MetricsReport,
    remote: &RemoteEventAgg,
    source: DataSource,
) -> MetricsReport {
    if source == DataSource::Local || remote.event_count == 0 {
        return report;
    }
    let token_map: HashMap<String, u64> = remote.tool_token_totals.iter().cloned().collect();
    for (tool, calls) in &remote.top_tools {
        let toks = token_map.get(tool).copied().unwrap_or(0);
        let row = RankedTool {
            tool: tool.clone(),
            calls: *calls,
            p50_ms: None,
            p95_ms: None,
            total_tokens: toks,
            total_reasoning_tokens: 0,
        };
        merge_one_tool_row(&mut report.slowest_tools, row.clone());
        merge_one_tool_row(&mut report.highest_token_tools, row);
    }
    report
        .slowest_tools
        .sort_by(|a, b| b.calls.cmp(&a.calls).then_with(|| a.tool.cmp(&b.tool)));
    report.highest_token_tools.sort_by(|a, b| {
        b.total_tokens
            .cmp(&a.total_tokens)
            .then_with(|| a.tool.cmp(&b.tool))
    });
    report.slowest_tools.truncate(10);
    report.highest_token_tools.truncate(10);
    report
}
