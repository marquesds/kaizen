// SPDX-License-Identifier: AGPL-3.0-or-later
//! WebSocket snapshot adapter for the read-only visualization screen.

use crate::core::paths;
use crate::store::Store;
use crate::visualization::{
    BuiltReport, SessionSearchInput, VisualizationLimits, VisualizationQuery, VisualizationReport,
    build_report_observed,
};
use anyhow::{Context, Result, ensure};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_SESSIONS: usize = 30;
const MAX_SELECTED_EVENTS: usize = 40;
const MAX_SELECTED_SPANS: usize = 40;
const MAX_SELECTED_FILES: usize = 40;
const MAX_SEARCH_CHARS: usize = 256;

pub struct SnapshotRequest {
    pub workspace: String,
    pub selected_session_id: Option<String>,
    pub q: String,
    pub offset: usize,
}

pub fn load(req: SnapshotRequest) -> Result<VisualizationReport> {
    let (store, key) = open_snapshot(&req.workspace)?;
    let search = search_input(&req.q, req.offset)?;
    let built = build_snapshot(&store, key, req.selected_session_id, search, now_ms())?;
    finish(built)
}

fn open_snapshot(raw: &str) -> Result<(Store, String)> {
    ensure!(!raw.trim().is_empty(), "workspace required");
    let root = workspace_root(raw)?;
    let store = Store::open_read_only(&database_path(&root)?)?;
    Ok((store, root.to_string_lossy().into_owned()))
}

fn database_path(root: &std::path::Path) -> Result<PathBuf> {
    let db = paths::project_data_path(root)?.join("kaizen.db");
    ensure!(
        db.is_file(),
        "no Kaizen data for {}; run `kaizen init --workspace {}`",
        root.display(),
        root.display()
    );
    Ok(db)
}

fn finish(built: BuiltReport) -> Result<VisualizationReport> {
    ensure_bounded(&built)?;
    let mut report = built.report;
    compact_report(&mut report);
    Ok(report)
}

fn workspace_root(raw: &str) -> Result<PathBuf> {
    let path = PathBuf::from(raw.trim());
    let root = std::fs::canonicalize(&path)
        .with_context(|| format!("workspace does not exist: {}", path.display()))?;
    ensure!(
        root.is_dir(),
        "workspace is not a directory: {}",
        path.display()
    );
    Ok(root)
}

fn build_snapshot(
    store: &Store,
    workspace: String,
    selected_session_id: Option<String>,
    session_search: SessionSearchInput,
    now_ms: u64,
) -> Result<BuiltReport> {
    build_report_observed(
        store,
        query(selected_session_id, workspace, session_search, now_ms),
    )
}

fn ensure_bounded(built: &BuiltReport) -> Result<()> {
    ensure_bound(built.materialized.sessions, MAX_SESSIONS, "session")?;
    ensure_bound(
        built.materialized.selected_events,
        MAX_SELECTED_EVENTS,
        "event",
    )?;
    ensure_bound(
        built.materialized.selected_spans,
        MAX_SELECTED_SPANS,
        "span",
    )?;
    ensure_bound(
        built.materialized.selected_files,
        MAX_SELECTED_FILES,
        "file",
    )?;
    Ok(())
}

fn ensure_bound(actual: usize, limit: usize, kind: &str) -> Result<()> {
    ensure!(actual <= limit, "Web {kind} read exceeded bound");
    Ok(())
}

fn query(
    selected_session_id: Option<String>,
    workspace: String,
    session_search: SessionSearchInput,
    now_ms: u64,
) -> VisualizationQuery {
    VisualizationQuery {
        workspace,
        selected_session_id,
        now_ms,
        include_activity: false,
        select_latest: true,
        session_search,
        limits: limits(),
    }
}

fn limits() -> VisualizationLimits {
    VisualizationLimits {
        sessions: MAX_SESSIONS,
        selected_events: MAX_SELECTED_EVENTS,
        selected_spans: MAX_SELECTED_SPANS,
        selected_files: MAX_SELECTED_FILES,
    }
}

fn search_input(raw: &str, offset: usize) -> Result<SessionSearchInput> {
    let q = raw.trim();
    ensure!(
        q.chars().count() <= MAX_SEARCH_CHARS,
        "search query exceeds 256 characters"
    );
    Ok(SessionSearchInput {
        q: q.into(),
        offset,
    })
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or_default()
}

fn compact_report(report: &mut VisualizationReport) {
    report.activity = Default::default();
    let Some(detail) = report.selected.as_mut() else {
        return;
    };
    super::event_display::prepare(detail);
}

#[cfg(test)]
mod tests;
