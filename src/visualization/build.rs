// SPDX-License-Identifier: AGPL-3.0-or-later

use super::activity::activity;
use super::types::*;
use crate::core::event::{SessionRecord, SessionStatus};
use crate::store::Store;
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

const ACTIVE_TTL_MS: u64 = 5 * 60_000;
const ORPHAN_TTL_MS: u64 = 30 * 60_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct VisualizationLimits {
    /// Maximum latest sessions materialized for summaries.
    pub sessions: usize,
    /// Maximum latest events materialized for selected detail.
    pub selected_events: usize,
    /// Maximum spans materialized for selected detail.
    pub selected_spans: usize,
    /// Maximum files materialized for selected detail.
    pub selected_files: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VisualizationQuery {
    pub workspace: String,
    pub selected_session_id: Option<String>,
    pub now_ms: u64,
    /// Compute heatmap bins only for surfaces that render them.
    pub include_activity: bool,
    /// Fall back to latest session when requested selection is absent or invalid.
    pub select_latest: bool,
    pub limits: VisualizationLimits,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MaterializedRows {
    pub(crate) sessions: usize,
    pub(crate) selected_events: usize,
    pub(crate) selected_spans: usize,
    pub(crate) selected_files: usize,
}

pub(crate) struct BuiltReport {
    pub(crate) report: VisualizationReport,
    pub(crate) materialized: MaterializedRows,
}

pub fn build_report(store: &Store, query: VisualizationQuery) -> Result<VisualizationReport> {
    Ok(build_report_observed(store, query)?.report)
}

pub(crate) fn build_report_observed(
    store: &Store,
    query: VisualizationQuery,
) -> Result<BuiltReport> {
    validate(&query.limits)?;
    let (totals, quality) = store.visualization_totals(&query.workspace)?;
    let sessions =
        store.visualization_sessions(&query.workspace, query.limits.sessions, query.now_ms)?;
    let selected = selected_detail(store, &query, sessions.first())?;
    let activity = activity_report(store, &query)?;
    let materialized = counts(&sessions, &selected);
    Ok(BuiltReport {
        report: report(query, totals, quality, sessions, selected, activity),
        materialized,
    })
}

fn validate(limits: &VisualizationLimits) -> Result<()> {
    ensure_positive(limits.sessions, "session")?;
    ensure_positive(limits.selected_events, "event")?;
    ensure_positive(limits.selected_spans, "span")?;
    ensure_positive(limits.selected_files, "file")?;
    Ok(())
}

fn ensure_positive(limit: usize, kind: &str) -> Result<()> {
    ensure!(limit > 0, "visualization {kind} limit must be positive");
    Ok(())
}

fn activity_report(store: &Store, query: &VisualizationQuery) -> Result<ActivityReport> {
    if query.include_activity {
        activity(store, &query.workspace, query.now_ms)
    } else {
        Ok(Default::default())
    }
}

fn selected_detail(
    store: &Store,
    query: &VisualizationQuery,
    latest: Option<&TraceSummary>,
) -> Result<Option<TraceDetail>> {
    let Some(session) = selected_session(store, query, latest)? else {
        return Ok(None);
    };
    let id = session.id.clone();
    Ok(Some(TraceDetail {
        session,
        events: store.list_latest_events_for_session(&id, query.limits.selected_events)?,
        spans: store.limited_session_span_tree(&id, query.limits.selected_spans)?,
        files: store.limited_files_for_session(&id, query.limits.selected_files)?,
    }))
}

fn selected_session(
    store: &Store,
    query: &VisualizationQuery,
    latest: Option<&TraceSummary>,
) -> Result<Option<SessionRecord>> {
    if let Some(session) = requested_session(store, query)? {
        return Ok(Some(session));
    }
    if !query.select_latest {
        return Ok(None);
    }
    latest
        .map(|summary| store.get_session(&summary.id))
        .transpose()
        .map(Option::flatten)
}

fn requested_session(store: &Store, query: &VisualizationQuery) -> Result<Option<SessionRecord>> {
    let Some(id) = query.selected_session_id.as_deref() else {
        return Ok(None);
    };
    Ok(store
        .get_session(id)?
        .filter(|session| session.workspace == query.workspace))
}

fn counts(sessions: &[TraceSummary], selected: &Option<TraceDetail>) -> MaterializedRows {
    MaterializedRows {
        sessions: sessions.len(),
        selected_events: selected.as_ref().map_or(0, |detail| detail.events.len()),
        selected_spans: selected
            .as_ref()
            .map_or(0, |detail| span_count(&detail.spans)),
        selected_files: selected.as_ref().map_or(0, |detail| detail.files.len()),
    }
}

fn span_count(spans: &[crate::store::SpanNode]) -> usize {
    spans
        .iter()
        .map(|node| 1 + span_count(&node.children))
        .sum()
}

fn report(
    query: VisualizationQuery,
    totals: VisualizationTotals,
    quality: DataQuality,
    sessions: Vec<TraceSummary>,
    selected: Option<TraceDetail>,
    activity: ActivityReport,
) -> VisualizationReport {
    VisualizationReport {
        generated_at_ms: query.now_ms,
        workspace: query.workspace,
        totals,
        activity,
        sessions,
        selected,
        quality,
    }
}

pub(crate) fn derive_status(
    session: &SessionRecord,
    last_event_ms: Option<u64>,
    error_count: u64,
    now_ms: u64,
) -> (DerivedStatus, String) {
    if error_count > 0 {
        return (DerivedStatus::Errored, "error event".into());
    }
    if session.status == SessionStatus::Done || session.ended_at_ms.is_some() {
        return (DerivedStatus::Done, "session ended".into());
    }
    match last_event_ms {
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
