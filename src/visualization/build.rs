// SPDX-License-Identifier: AGPL-3.0-or-later

use super::activity::activity;
use super::rollup::{cost_session_count, counts, has_tokens, pct, token_totals};
use super::types::*;
use crate::core::event::{Event, EventKind, SessionRecord, SessionStatus};
use crate::store::Store;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const ACTIVE_TTL_MS: u64 = 5 * 60_000;
const ORPHAN_TTL_MS: u64 = 30 * 60_000;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VisualizationQuery {
    pub workspace: String,
    pub selected_session_id: Option<String>,
    pub now_ms: u64,
    pub day_start_hour: u8,
}

pub fn build_report(store: &Store, query: VisualizationQuery) -> Result<VisualizationReport> {
    let sessions = store.list_sessions(&query.workspace)?;
    let pairs = store.workspace_events(&query.workspace)?;
    let selected = selected_detail(store, &query)?;
    let summaries = trace_summaries(&sessions, &pairs, query.now_ms);
    Ok(VisualizationReport {
        generated_at_ms: query.now_ms,
        workspace: query.workspace,
        totals: totals(&sessions, &pairs),
        activity: activity(&pairs, query.now_ms),
        sessions: summaries,
        selected,
        quality: quality(&sessions, &pairs),
    })
}

fn selected_detail(store: &Store, query: &VisualizationQuery) -> Result<Option<TraceDetail>> {
    let Some(id) = query.selected_session_id.as_deref() else {
        return Ok(None);
    };
    let Some(session) = store.get_session(id)? else {
        return Ok(None);
    };
    Ok(Some(TraceDetail {
        session,
        events: store.list_events_for_session(id)?,
        spans: store.session_span_tree(id)?,
        files: store.files_for_session(id)?,
    }))
}

fn totals(sessions: &[SessionRecord], pairs: &[(SessionRecord, Event)]) -> VisualizationTotals {
    let tokens = token_totals(pairs.iter().map(|(_, e)| e));
    VisualizationTotals {
        session_count: sessions.len() as u64,
        running_count: sessions
            .iter()
            .filter(|s| s.status != SessionStatus::Done)
            .count() as u64,
        event_count: pairs.len() as u64,
        error_count: pairs
            .iter()
            .filter(|(_, e)| e.kind == EventKind::Error)
            .count() as u64,
        tool_call_count: pairs
            .iter()
            .filter(|(_, e)| e.kind == EventKind::ToolCall)
            .count() as u64,
        cost_usd_e6: pairs.iter().filter_map(|(_, e)| e.cost_usd_e6).sum(),
        tokens,
    }
}

fn trace_summaries(
    sessions: &[SessionRecord],
    pairs: &[(SessionRecord, Event)],
    now_ms: u64,
) -> Vec<TraceSummary> {
    let grouped = group_events(pairs);
    sessions
        .iter()
        .take(100)
        .map(|s| {
            trace_summary(
                s,
                grouped.get(s.id.as_str()).cloned().unwrap_or_default(),
                now_ms,
            )
        })
        .collect()
}

fn trace_summary(session: &SessionRecord, events: Vec<&Event>, now_ms: u64) -> TraceSummary {
    let (status, status_reason) = derived_status(session, &events, now_ms);
    TraceSummary {
        id: session.id.clone(),
        agent: session.agent.clone(),
        model: session.model.clone(),
        status,
        status_reason,
        started_at_ms: session.started_at_ms,
        ended_at_ms: session.ended_at_ms,
        last_event_ms: events.iter().map(|e| e.ts_ms).max(),
        event_count: events.len() as u64,
        error_count: events.iter().filter(|e| e.kind == EventKind::Error).count() as u64,
        tool_call_count: events
            .iter()
            .filter(|e| e.kind == EventKind::ToolCall)
            .count() as u64,
        cost_usd_e6: events.iter().filter_map(|e| e.cost_usd_e6).sum(),
        tokens: token_totals(events.iter().copied()),
        top_tools: top_tools(&events),
    }
}

fn quality(sessions: &[SessionRecord], pairs: &[(SessionRecord, Event)]) -> DataQuality {
    let token_events = pairs.iter().filter(|(_, e)| has_tokens(e)).count();
    let cost_events = pairs
        .iter()
        .filter(|(_, e)| e.cost_usd_e6.is_some())
        .count();
    DataQuality {
        token_coverage_pct: pct(pairs.len(), token_events),
        cost_coverage_pct: pct(pairs.len(), cost_events),
        partial_cost_sessions: sessions.len().saturating_sub(cost_session_count(pairs)) as u64,
        warnings: empty_warnings(sessions, pairs),
        ..DataQuality::default()
    }
}

fn derived_status(s: &SessionRecord, events: &[&Event], now_ms: u64) -> (DerivedStatus, String) {
    if events.iter().any(|e| e.kind == EventKind::Error) {
        return (DerivedStatus::Errored, "error event".into());
    }
    if s.status == SessionStatus::Done || s.ended_at_ms.is_some() {
        return (DerivedStatus::Done, "session ended".into());
    }
    match events.iter().map(|e| e.ts_ms).max() {
        Some(ts) if now_ms.saturating_sub(ts) <= ACTIVE_TTL_MS => {
            (DerivedStatus::Active, "recent event".into())
        }
        Some(ts) if now_ms.saturating_sub(ts) >= ORPHAN_TTL_MS => {
            (DerivedStatus::Orphaned, "stale open session".into())
        }
        Some(_) => (DerivedStatus::Idle, "no recent event".into()),
        None => (DerivedStatus::Idle, "no events".into()),
    }
}

fn group_events(pairs: &[(SessionRecord, Event)]) -> HashMap<&str, Vec<&Event>> {
    let mut out: HashMap<&str, Vec<&Event>> = HashMap::new();
    pairs
        .iter()
        .for_each(|(s, e)| out.entry(s.id.as_str()).or_default().push(e));
    out
}

fn top_tools(events: &[&Event]) -> Vec<(String, u64)> {
    let mut counts = counts(events.iter().filter_map(|e| e.tool.as_deref()));
    counts.truncate(5);
    counts
}

fn empty_warnings(sessions: &[SessionRecord], pairs: &[(SessionRecord, Event)]) -> Vec<String> {
    (sessions.is_empty() || pairs.is_empty())
        .then(|| "no local telemetry for workspace".to_string())
        .into_iter()
        .collect()
}
