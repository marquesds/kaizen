// SPDX-License-Identifier: AGPL-3.0-or-later
//! Sync SQLite store. WAL mode, schema migrations as ordered SQL strings.

use crate::core::config::try_team_salt;
use crate::core::event::{Event, EventKind, EventSource, SessionRecord, SessionStatus};
use crate::core::trace_span::{TraceSpanKind, TraceSpanRecord};
use crate::metrics::types::{
    FileFact, RankedFile, RankedTool, RepoEdge, RepoSnapshotRecord, ToolSpanView,
};
use crate::store::event_index::index_event_derived;
use crate::store::projector::{DEFAULT_ORPHAN_TTL_MS, Projector, ProjectorEvent};
use crate::store::tool_span_index::{
    clear_session_spans, rebuild_tool_spans_for_session, upsert_tool_span_record,
};
use crate::store::{hot_log::HotLog, outbox_redb::Outbox};
use crate::sync::context::SyncIngestContext;
use crate::sync::outbound::outbound_event_from_row;
use crate::sync::redact::redact_payload;
use crate::sync::smart::enqueue_tool_spans_for_session;
use anyhow::{Context, Result};
use rusqlite::types::Value;
use rusqlite::{
    Connection, OpenFlags, OptionalExtension, TransactionBehavior, params, params_from_iter,
};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Max `ts_ms` still treated as transcript-only synthetic timing (seq-based fallbacks).
/// Rows below this use `sessions.started_at_ms` for time-window matching.
pub(super) const SYNTHETIC_TS_CEILING_MS: i64 = 1_000_000_000_000;
pub(super) const DEFAULT_MMAP_MB: u64 = 256;
pub(super) const SESSION_SELECT: &str =
    "SELECT id, agent, model, workspace, started_at_ms, ended_at_ms,
    status, trace_path, start_commit, end_commit, branch, dirty_start, dirty_end,
    repo_binding_source, prompt_fingerprint, parent_session_id, agent_version, os, arch,
    repo_file_count, repo_total_loc FROM sessions";
pub(super) const PAIN_HOTSPOTS_SQL: &str = "
    SELECT f.path,
           COUNT(s.id) * f.complexity_total AS value,
           f.complexity_total,
           f.churn_30d
    FROM file_facts f
    LEFT JOIN tool_span_paths tsp ON tsp.path = f.path
    LEFT JOIN tool_spans ts ON ts.span_id = tsp.span_id
       AND ((ts.started_at_ms >= ?3 AND ts.started_at_ms <= ?4)
         OR (ts.started_at_ms IS NULL AND ts.ended_at_ms >= ?3 AND ts.ended_at_ms <= ?4))
    LEFT JOIN sessions s ON s.id = ts.session_id AND s.workspace = ?2
    WHERE f.snapshot_id = ?1
    GROUP BY f.path, f.complexity_total, f.churn_30d
    ORDER BY value DESC, f.path ASC
    LIMIT 10";
pub(super) const TOOL_RANK_ROWS_SQL: &str = "
    WITH scoped AS (
      SELECT COALESCE(ts.tool, 'unknown') AS tool,
             ts.lead_time_ms,
             COALESCE(ts.tokens_in, 0) + COALESCE(ts.tokens_out, 0)
                 + COALESCE(ts.reasoning_tokens, 0) AS total_tokens,
             COALESCE(ts.reasoning_tokens, 0) AS reasoning_tokens
      FROM tool_spans ts
      JOIN sessions s ON s.id = ts.session_id
      WHERE s.workspace = ?1
        AND ((ts.started_at_ms >= ?2 AND ts.started_at_ms <= ?3)
          OR (ts.started_at_ms IS NULL AND ts.ended_at_ms >= ?2 AND ts.ended_at_ms <= ?3))
    ),
    agg AS (
      SELECT tool, COUNT(*) AS calls, SUM(total_tokens) AS total_tokens,
             SUM(reasoning_tokens) AS total_reasoning_tokens
      FROM scoped GROUP BY tool
    ),
    lat AS (
      SELECT tool, lead_time_ms,
             ROW_NUMBER() OVER (PARTITION BY tool ORDER BY lead_time_ms) AS rn,
             COUNT(*) OVER (PARTITION BY tool) AS n
      FROM scoped WHERE lead_time_ms IS NOT NULL
    ),
    pct AS (
      SELECT tool,
             MAX(CASE WHEN rn = CAST(((n - 1) * 50) / 100 AS INTEGER) + 1 THEN lead_time_ms END) AS p50_ms,
             MAX(CASE WHEN rn = CAST(((n - 1) * 95) / 100 AS INTEGER) + 1 THEN lead_time_ms END) AS p95_ms
      FROM lat GROUP BY tool
    )
    SELECT agg.tool, agg.calls, pct.p50_ms, pct.p95_ms,
           agg.total_tokens, agg.total_reasoning_tokens
    FROM agg LEFT JOIN pct ON pct.tool = agg.tool";

/// Per-workspace activity dashboard stats.
#[derive(Clone)]
pub struct InsightsStats {
    pub total_sessions: u64,
    pub running_sessions: u64,
    pub total_events: u64,
    /// (day label e.g. "Mon", count) last 7 days oldest first
    pub sessions_by_day: Vec<(String, u64)>,
    /// Recent sessions DESC by started_at, max 3; paired with event count
    pub recent: Vec<(SessionRecord, u64)>,
    /// Top tools by event count, max 5
    pub top_tools: Vec<(String, u64)>,
    pub total_cost_usd_e6: i64,
    pub sessions_with_cost: u64,
}

/// Sync daemon / outbox status for `kaizen sync status`.
pub struct SyncStatusSnapshot {
    pub pending_outbox: u64,
    pub last_success_ms: Option<u64>,
    pub last_error: Option<String>,
    pub consecutive_failures: u32,
}

/// Aggregate stats across sessions + events for a workspace.
#[derive(serde::Serialize)]
pub struct SummaryStats {
    pub session_count: u64,
    pub total_cost_usd_e6: i64,
    pub by_agent: Vec<(String, u64)>,
    pub by_model: Vec<(String, u64)>,
    pub top_tools: Vec<(String, u64)>,
}

/// Skill vs Cursor rule for [`GuidancePerfRow`].
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GuidanceKind {
    Skill,
    Rule,
}

/// One row for `kaizen guidance` — observed references in payloads (not Cursor auto-load counts).
#[derive(Clone, Debug, serde::Serialize)]
pub struct GuidancePerfRow {
    pub kind: GuidanceKind,
    pub id: String,
    pub sessions: u64,
    pub sessions_pct: f64,
    pub total_cost_usd_e6: i64,
    pub avg_cost_per_session_usd: Option<f64>,
    pub vs_workspace_avg_cost_per_session_usd: Option<f64>,
    pub on_disk: bool,
}

/// Aggregated skill/rule adoption and cost proxy for a time window.
#[derive(Clone, Debug, serde::Serialize)]
pub struct GuidanceReport {
    pub workspace: String,
    pub window_start_ms: u64,
    pub window_end_ms: u64,
    pub sessions_in_window: u64,
    pub workspace_avg_cost_per_session_usd: Option<f64>,
    pub rows: Vec<GuidancePerfRow>,
}

/// Result of [`Store::prune_sessions_started_before`].
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub struct PruneStats {
    pub sessions_removed: u64,
    pub events_removed: u64,
}

/// Row in `session_outcomes` (Tier C — post-stop test/lint snapshot).
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SessionOutcomeRow {
    pub session_id: String,
    pub test_passed: Option<i64>,
    pub test_failed: Option<i64>,
    pub test_skipped: Option<i64>,
    pub build_ok: Option<bool>,
    pub lint_errors: Option<i64>,
    pub revert_lines_14d: Option<i64>,
    pub pr_open: Option<i64>,
    pub ci_ok: Option<bool>,
    pub measured_at_ms: u64,
    pub measure_error: Option<String>,
}

/// Aggregated process samples for retro (Tier D).
#[derive(Debug, Clone)]
pub struct SessionSampleAgg {
    pub session_id: String,
    pub sample_count: u64,
    pub max_cpu_percent: f64,
    pub max_rss_bytes: u64,
}

/// `sync_state` keys for agent rescan throttling and auto-prune.
pub const SYNC_STATE_LAST_AGENT_SCAN_MS: &str = "last_agent_scan_ms";
pub const SYNC_STATE_LAST_AUTO_PRUNE_MS: &str = "last_auto_prune_ms";
pub const SYNC_STATE_SEARCH_DIRTY_MS: &str = "search_dirty_ms";

pub struct ToolSpanSyncRow {
    pub span_id: String,
    pub session_id: String,
    pub tool: Option<String>,
    pub tool_call_id: Option<String>,
    pub status: String,
    pub started_at_ms: Option<u64>,
    pub ended_at_ms: Option<u64>,
    pub lead_time_ms: Option<u64>,
    pub tokens_in: Option<u32>,
    pub tokens_out: Option<u32>,
    pub reasoning_tokens: Option<u32>,
    pub cost_usd_e6: Option<i64>,
    pub paths: Vec<String>,
}

pub(crate) struct CaptureQualityRow {
    pub source: String,
    pub has_tokens: bool,
    pub has_latency: bool,
    pub has_context: bool,
}

pub(crate) struct TraceSpanQualityRow {
    pub kind: String,
    pub is_orphan: bool,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum StoreOpenMode {
    ReadWrite,
    ReadOnlyQuery,
}

#[derive(Debug, Clone)]
pub struct SessionStatusRow {
    pub id: String,
    pub status: SessionStatus,
    pub ended_at_ms: Option<u64>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct SessionFilter {
    pub agent_prefix: Option<String>,
    pub status: Option<SessionStatus>,
    pub since_ms: Option<u64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SessionPage {
    pub rows: Vec<SessionRecord>,
    pub total: usize,
    pub next_offset: Option<usize>,
}

#[derive(Clone)]
struct SpanTreeCacheEntry {
    session_id: String,
    last_event_seq: Option<u64>,
    nodes: Vec<crate::store::span_tree::SpanNode>,
}

pub struct Store {
    conn: Connection,
    root: PathBuf,
    hot_log: RefCell<Option<HotLog>>,
    search_writer: RefCell<Option<crate::search::PendingWriter>>,
    span_tree_cache: RefCell<Option<SpanTreeCacheEntry>>,
    projector: RefCell<Projector>,
}

mod artifact_windows;
mod evals;
mod event_projector;
mod event_read;
mod event_write;
mod events;
mod experiment_windows;
mod feedback;
mod guidance;
mod guidance_candidates;
mod maintenance;
mod metrics;
mod outcomes;
mod prompts;
mod report_windows;
mod reports;
mod rows;
mod samples;
mod schema;
mod session_read;
mod session_window;
mod sessions;
mod sync;
#[cfg(test)]
mod tests;
mod tool_span_sync;
mod tool_spans;
mod trace_spans;

pub(super) fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

impl Store {
    pub(crate) fn conn(&self) -> &Connection {
        &self.conn
    }

    pub fn open(path: &Path) -> Result<Self> {
        Self::open_with_mode(path, StoreOpenMode::ReadWrite)
    }

    pub fn open_read_only(path: &Path) -> Result<Self> {
        Self::open_with_mode(path, StoreOpenMode::ReadOnlyQuery)
    }

    pub fn open_query(path: &Path) -> Result<Self> {
        Self::open_with_mode(path, StoreOpenMode::ReadOnlyQuery)
    }

    pub fn open_with_mode(path: &Path, mode: StoreOpenMode) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = match mode {
            StoreOpenMode::ReadWrite => Connection::open(path),
            StoreOpenMode::ReadOnlyQuery => Connection::open_with_flags(
                path,
                OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            ),
        }
        .with_context(|| format!("open db: {}", path.display()))?;
        schema::apply_pragmas(&conn, mode)?;
        if mode == StoreOpenMode::ReadWrite {
            for sql in schema::MIGRATIONS {
                conn.execute_batch(sql)?;
            }
            schema::ensure_schema_columns(&conn)?;
        }
        let store = Self {
            conn,
            root: path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .to_path_buf(),
            hot_log: RefCell::new(None),
            search_writer: RefCell::new(None),
            span_tree_cache: RefCell::new(None),
            projector: RefCell::new(Projector::default()),
        };
        if mode == StoreOpenMode::ReadWrite {
            store.warm_projector()?;
        }
        Ok(store)
    }

    pub(super) fn invalidate_span_tree_cache(&self) {
        *self.span_tree_cache.borrow_mut() = None;
    }

    pub(super) fn warm_projector(&self) -> Result<()> {
        let ids = self.running_session_ids()?;
        let mut projector = self.projector.borrow_mut();
        for id in ids {
            for event in self.list_events_for_session(&id)? {
                let _ = projector.apply(&event);
            }
        }
        Ok(())
    }

    pub(super) fn outbox(&self) -> Result<Outbox> {
        Outbox::open(&self.root)
    }
}

impl Drop for Store {
    fn drop(&mut self) {
        if let Some(writer) = self.search_writer.get_mut().as_mut() {
            let _ = writer.commit();
        }
    }
}
