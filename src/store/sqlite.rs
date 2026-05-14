// SPDX-License-Identifier: AGPL-3.0-or-later
//! Sync SQLite store. WAL mode, schema migrations as ordered SQL strings.

use crate::core::config::try_team_salt;
use crate::core::event::{Event, EventKind, EventSource, SessionRecord, SessionStatus};
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
const SYNTHETIC_TS_CEILING_MS: i64 = 1_000_000_000_000;
const DEFAULT_MMAP_MB: u64 = 256;
const SESSION_SELECT: &str = "SELECT id, agent, model, workspace, started_at_ms, ended_at_ms,
    status, trace_path, start_commit, end_commit, branch, dirty_start, dirty_end,
    repo_binding_source, prompt_fingerprint, parent_session_id, agent_version, os, arch,
    repo_file_count, repo_total_loc FROM sessions";
const PAIN_HOTSPOTS_SQL: &str = "
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
const TOOL_RANK_ROWS_SQL: &str = "
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

const MIGRATIONS: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS sessions (
        id TEXT PRIMARY KEY,
        agent TEXT NOT NULL,
        model TEXT,
        workspace TEXT NOT NULL,
        started_at_ms INTEGER NOT NULL,
        ended_at_ms INTEGER,
        status TEXT NOT NULL,
        trace_path TEXT NOT NULL
    )",
    "CREATE TABLE IF NOT EXISTS events (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        session_id TEXT NOT NULL,
        seq INTEGER NOT NULL,
        ts_ms INTEGER NOT NULL,
        kind TEXT NOT NULL,
        source TEXT NOT NULL,
        tool TEXT,
        tokens_in INTEGER,
        tokens_out INTEGER,
        cost_usd_e6 INTEGER,
        payload TEXT NOT NULL
    )",
    "CREATE INDEX IF NOT EXISTS events_session_idx ON events(session_id)",
    "CREATE TABLE IF NOT EXISTS files_touched (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        session_id TEXT NOT NULL,
        path TEXT NOT NULL
    )",
    "CREATE TABLE IF NOT EXISTS skills_used (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        session_id TEXT NOT NULL,
        skill TEXT NOT NULL
    )",
    "CREATE TABLE IF NOT EXISTS sync_outbox (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        session_id TEXT NOT NULL,
        payload TEXT NOT NULL,
        sent INTEGER NOT NULL DEFAULT 0
    )",
    "CREATE TABLE IF NOT EXISTS experiments (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        created_at_ms INTEGER NOT NULL,
        metadata TEXT NOT NULL DEFAULT '{}'
    )",
    "CREATE TABLE IF NOT EXISTS experiment_tags (
        experiment_id TEXT NOT NULL,
        session_id TEXT NOT NULL,
        variant TEXT NOT NULL,
        PRIMARY KEY (experiment_id, session_id)
    )",
    "CREATE UNIQUE INDEX IF NOT EXISTS events_session_seq_idx ON events(session_id, seq)",
    "CREATE TABLE IF NOT EXISTS sync_state (
        k TEXT PRIMARY KEY,
        v TEXT NOT NULL
    )",
    "CREATE UNIQUE INDEX IF NOT EXISTS files_touched_session_path_idx ON files_touched(session_id, path)",
    "CREATE UNIQUE INDEX IF NOT EXISTS skills_used_session_skill_idx ON skills_used(session_id, skill)",
    "CREATE TABLE IF NOT EXISTS tool_spans (
        span_id TEXT PRIMARY KEY,
        session_id TEXT NOT NULL,
        tool TEXT,
        tool_call_id TEXT,
        status TEXT NOT NULL,
        started_at_ms INTEGER,
        ended_at_ms INTEGER,
        lead_time_ms INTEGER,
        tokens_in INTEGER,
        tokens_out INTEGER,
        reasoning_tokens INTEGER,
        cost_usd_e6 INTEGER,
        paths_json TEXT NOT NULL DEFAULT '[]'
    )",
    "CREATE TABLE IF NOT EXISTS tool_span_paths (
        span_id TEXT NOT NULL,
        path TEXT NOT NULL,
        PRIMARY KEY (span_id, path)
    )",
    "CREATE TABLE IF NOT EXISTS session_repo_binding (
        session_id TEXT PRIMARY KEY,
        start_commit TEXT,
        end_commit TEXT,
        branch TEXT,
        dirty_start INTEGER,
        dirty_end INTEGER,
        repo_binding_source TEXT NOT NULL DEFAULT ''
    )",
    "CREATE TABLE IF NOT EXISTS repo_snapshots (
        id TEXT PRIMARY KEY,
        workspace TEXT NOT NULL,
        head_commit TEXT,
        dirty_fingerprint TEXT NOT NULL,
        analyzer_version TEXT NOT NULL,
        indexed_at_ms INTEGER NOT NULL,
        dirty INTEGER NOT NULL DEFAULT 0,
        graph_path TEXT NOT NULL
    )",
    "CREATE TABLE IF NOT EXISTS file_facts (
        snapshot_id TEXT NOT NULL,
        path TEXT NOT NULL,
        language TEXT NOT NULL,
        bytes INTEGER NOT NULL,
        loc INTEGER NOT NULL,
        sloc INTEGER NOT NULL,
        complexity_total INTEGER NOT NULL,
        max_fn_complexity INTEGER NOT NULL,
        symbol_count INTEGER NOT NULL,
        import_count INTEGER NOT NULL,
        fan_in INTEGER NOT NULL,
        fan_out INTEGER NOT NULL,
        churn_30d INTEGER NOT NULL,
        churn_90d INTEGER NOT NULL,
        authors_90d INTEGER NOT NULL,
        last_changed_ms INTEGER,
        PRIMARY KEY (snapshot_id, path)
    )",
    "CREATE TABLE IF NOT EXISTS repo_edges (
        snapshot_id TEXT NOT NULL,
        from_id TEXT NOT NULL,
        to_id TEXT NOT NULL,
        kind TEXT NOT NULL,
        weight INTEGER NOT NULL,
        PRIMARY KEY (snapshot_id, from_id, to_id, kind)
    )",
    // Speed workspace-scoped `insights` / `summary` (sessions filter before joining events)
    "CREATE INDEX IF NOT EXISTS sessions_workspace_idx ON sessions(workspace)",
    // `ORDER BY started_at_ms` for a workspace (list_sessions, recent_sessions_3)
    "CREATE INDEX IF NOT EXISTS sessions_workspace_started_idx ON sessions(workspace, started_at_ms)",
    "CREATE INDEX IF NOT EXISTS sessions_workspace_started_desc_idx
        ON sessions(workspace, started_at_ms DESC, id ASC)",
    "CREATE INDEX IF NOT EXISTS sessions_workspace_agent_lower_idx
        ON sessions(workspace, lower(agent), started_at_ms DESC, id ASC)",
    "CREATE TABLE IF NOT EXISTS rules_used (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        session_id TEXT NOT NULL,
        rule TEXT NOT NULL
    )",
    "CREATE UNIQUE INDEX IF NOT EXISTS rules_used_session_rule_idx ON rules_used(session_id, rule)",
    // Provider pull cache (single-row state + per-kind rows; atomic refresh = txn + clear + insert)
    "CREATE TABLE IF NOT EXISTS remote_pull_state (
        id INTEGER PRIMARY KEY CHECK (id = 1),
        query_provider TEXT NOT NULL DEFAULT 'none',
        cursor_json TEXT NOT NULL DEFAULT '',
        last_success_ms INTEGER
    )",
    "INSERT OR IGNORE INTO remote_pull_state (id) VALUES (1)",
    "CREATE TABLE IF NOT EXISTS remote_sessions (
        team_id TEXT NOT NULL,
        workspace_hash TEXT NOT NULL,
        session_id_hash TEXT NOT NULL,
        json TEXT NOT NULL,
        PRIMARY KEY (team_id, workspace_hash, session_id_hash)
    )",
    "CREATE TABLE IF NOT EXISTS remote_events (
        team_id TEXT NOT NULL,
        workspace_hash TEXT NOT NULL,
        session_id_hash TEXT NOT NULL,
        event_seq INTEGER NOT NULL,
        json TEXT NOT NULL,
        PRIMARY KEY (team_id, workspace_hash, session_id_hash, event_seq)
    )",
    "CREATE TABLE IF NOT EXISTS remote_tool_spans (
        team_id TEXT NOT NULL,
        workspace_hash TEXT NOT NULL,
        span_id_hash TEXT NOT NULL,
        json TEXT NOT NULL,
        PRIMARY KEY (team_id, workspace_hash, span_id_hash)
    )",
    "CREATE TABLE IF NOT EXISTS remote_repo_snapshots (
        team_id TEXT NOT NULL,
        workspace_hash TEXT NOT NULL,
        snapshot_id_hash TEXT NOT NULL,
        chunk_index INTEGER NOT NULL,
        json TEXT NOT NULL,
        PRIMARY KEY (team_id, workspace_hash, snapshot_id_hash, chunk_index)
    )",
    "CREATE TABLE IF NOT EXISTS remote_workspace_facts (
        team_id TEXT NOT NULL,
        workspace_hash TEXT NOT NULL,
        fact_key TEXT NOT NULL,
        json TEXT NOT NULL,
        PRIMARY KEY (team_id, workspace_hash, fact_key)
    )",
    "CREATE TABLE IF NOT EXISTS session_evals (
        id            TEXT    PRIMARY KEY,
        session_id    TEXT    NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
        judge_model   TEXT    NOT NULL,
        rubric_id     TEXT    NOT NULL,
        score         REAL    NOT NULL CHECK(score BETWEEN 0.0 AND 1.0),
        rationale     TEXT    NOT NULL,
        flagged       INTEGER NOT NULL DEFAULT 0,
        created_at_ms INTEGER NOT NULL
    );
    CREATE INDEX IF NOT EXISTS session_evals_session ON session_evals(session_id);
    CREATE INDEX IF NOT EXISTS session_evals_rubric  ON session_evals(rubric_id, score)",
    "CREATE TABLE IF NOT EXISTS prompt_snapshots (
        fingerprint   TEXT    PRIMARY KEY,
        captured_at_ms INTEGER NOT NULL,
        files_json    TEXT    NOT NULL,
        total_bytes   INTEGER NOT NULL
    )",
    "CREATE TABLE IF NOT EXISTS session_feedback (
        id TEXT PRIMARY KEY,
        session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
        score INTEGER CHECK(score BETWEEN 1 AND 5),
        label TEXT CHECK(label IN ('good','bad','interesting','bug','regression')),
        note TEXT,
        created_at_ms INTEGER NOT NULL
    );
    CREATE INDEX IF NOT EXISTS session_feedback_session ON session_feedback(session_id);
    CREATE INDEX IF NOT EXISTS session_feedback_label ON session_feedback(label, created_at_ms)",
    "CREATE TABLE IF NOT EXISTS session_outcomes (
        session_id TEXT PRIMARY KEY NOT NULL,
        test_passed INTEGER,
        test_failed INTEGER,
        test_skipped INTEGER,
        build_ok INTEGER,
        lint_errors INTEGER,
        revert_lines_14d INTEGER,
        pr_open INTEGER,
        ci_ok INTEGER,
        measured_at_ms INTEGER NOT NULL,
        measure_error TEXT
    )",
    "CREATE TABLE IF NOT EXISTS session_samples (
        session_id TEXT NOT NULL,
        ts_ms INTEGER NOT NULL,
        pid INTEGER NOT NULL,
        cpu_percent REAL,
        rss_bytes INTEGER,
        PRIMARY KEY (session_id, ts_ms, pid)
    )",
    "CREATE INDEX IF NOT EXISTS session_samples_session_idx ON session_samples(session_id)",
    "CREATE INDEX IF NOT EXISTS tool_spans_session_idx ON tool_spans(session_id)",
    "CREATE INDEX IF NOT EXISTS tool_spans_started_idx ON tool_spans(started_at_ms)",
    "CREATE INDEX IF NOT EXISTS tool_spans_ended_idx ON tool_spans(ended_at_ms)",
    "CREATE INDEX IF NOT EXISTS session_samples_ts_idx ON session_samples(ts_ms)",
    "CREATE INDEX IF NOT EXISTS events_ts_idx ON events(ts_ms)",
    "CREATE INDEX IF NOT EXISTS events_ts_session_seq_idx ON events(ts_ms, session_id, seq)",
    "CREATE INDEX IF NOT EXISTS events_session_ts_seq_idx ON events(session_id, ts_ms, seq)",
    "CREATE INDEX IF NOT EXISTS events_tool_ts_session_seq_idx ON events(tool, ts_ms DESC, session_id, seq)",
    "CREATE INDEX IF NOT EXISTS tool_spans_session_started_idx ON tool_spans(session_id, started_at_ms)",
    "CREATE INDEX IF NOT EXISTS tool_spans_session_ended_idx ON tool_spans(session_id, ended_at_ms)",
    "CREATE INDEX IF NOT EXISTS tool_span_paths_path_idx ON tool_span_paths(path, span_id)",
    "CREATE INDEX IF NOT EXISTS feedback_session_idx ON session_feedback(session_id)",
];

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
        apply_pragmas(&conn, mode)?;
        if mode == StoreOpenMode::ReadWrite {
            for sql in MIGRATIONS {
                conn.execute_batch(sql)?;
            }
            ensure_schema_columns(&conn)?;
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

    fn invalidate_span_tree_cache(&self) {
        *self.span_tree_cache.borrow_mut() = None;
    }

    fn warm_projector(&self) -> Result<()> {
        let ids = self.running_session_ids()?;
        let mut projector = self.projector.borrow_mut();
        for id in ids {
            for event in self.list_events_for_session(&id)? {
                let _ = projector.apply(&event);
            }
        }
        Ok(())
    }

    pub fn upsert_session(&self, s: &SessionRecord) -> Result<()> {
        self.conn.execute(
            "INSERT INTO sessions (
                id, agent, model, workspace, started_at_ms, ended_at_ms, status, trace_path,
                start_commit, end_commit, branch, dirty_start, dirty_end, repo_binding_source,
                prompt_fingerprint, parent_session_id, agent_version, os, arch,
                repo_file_count, repo_total_loc
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15,
                ?16, ?17, ?18, ?19, ?20, ?21)
             ON CONFLICT(id) DO UPDATE SET
               agent=excluded.agent, model=excluded.model, workspace=excluded.workspace,
               started_at_ms=excluded.started_at_ms, ended_at_ms=excluded.ended_at_ms,
               status=excluded.status, trace_path=excluded.trace_path,
               start_commit=excluded.start_commit, end_commit=excluded.end_commit,
               branch=excluded.branch, dirty_start=excluded.dirty_start,
               dirty_end=excluded.dirty_end, repo_binding_source=excluded.repo_binding_source,
               prompt_fingerprint=excluded.prompt_fingerprint,
               parent_session_id=excluded.parent_session_id,
               agent_version=excluded.agent_version, os=excluded.os, arch=excluded.arch,
               repo_file_count=excluded.repo_file_count, repo_total_loc=excluded.repo_total_loc",
            params![
                s.id,
                s.agent,
                s.model,
                s.workspace,
                s.started_at_ms as i64,
                s.ended_at_ms.map(|v| v as i64),
                format!("{:?}", s.status),
                s.trace_path,
                s.start_commit,
                s.end_commit,
                s.branch,
                s.dirty_start.map(bool_to_i64),
                s.dirty_end.map(bool_to_i64),
                s.repo_binding_source.clone().unwrap_or_default(),
                s.prompt_fingerprint.as_deref(),
                s.parent_session_id.as_deref(),
                s.agent_version.as_deref(),
                s.os.as_deref(),
                s.arch.as_deref(),
                s.repo_file_count.map(|v| v as i64),
                s.repo_total_loc.map(|v| v as i64),
            ],
        )?;
        self.conn.execute(
            "INSERT INTO session_repo_binding (
                session_id, start_commit, end_commit, branch, dirty_start, dirty_end, repo_binding_source
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(session_id) DO UPDATE SET
                start_commit=excluded.start_commit,
                end_commit=excluded.end_commit,
                branch=excluded.branch,
                dirty_start=excluded.dirty_start,
                dirty_end=excluded.dirty_end,
                repo_binding_source=excluded.repo_binding_source",
            params![
                s.id,
                s.start_commit,
                s.end_commit,
                s.branch,
                s.dirty_start.map(bool_to_i64),
                s.dirty_end.map(bool_to_i64),
                s.repo_binding_source.clone().unwrap_or_default(),
            ],
        )?;
        Ok(())
    }

    /// Insert a minimal session row if none exists. Used by hook ingestion when
    /// the first observed event is not `SessionStart` (hooks installed mid-session).
    pub fn ensure_session_stub(
        &self,
        id: &str,
        agent: &str,
        workspace: &str,
        started_at_ms: u64,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO sessions (
                id, agent, model, workspace, started_at_ms, ended_at_ms, status, trace_path,
                start_commit, end_commit, branch, dirty_start, dirty_end, repo_binding_source,
                prompt_fingerprint, parent_session_id, agent_version, os, arch, repo_file_count, repo_total_loc
             ) VALUES (?1, ?2, NULL, ?3, ?4, NULL, 'Running', '', NULL, NULL, NULL, NULL, NULL, '',
                NULL, NULL, NULL, NULL, NULL, NULL, NULL)",
            params![id, agent, workspace, started_at_ms as i64],
        )?;
        Ok(())
    }

    /// Next `seq` for a new event in this session (0 when there are no events yet).
    pub fn next_event_seq(&self, session_id: &str) -> Result<u64> {
        let n: i64 = self.conn.query_row(
            "SELECT COALESCE(MAX(seq) + 1, 0) FROM events WHERE session_id = ?1",
            [session_id],
            |r| r.get(0),
        )?;
        Ok(n as u64)
    }

    pub fn append_event(&self, e: &Event) -> Result<()> {
        self.append_event_with_sync(e, None)
    }

    /// Append event; when `ctx` is set and sync is configured, enqueue one redacted outbox row.
    pub fn append_event_with_sync(&self, e: &Event, ctx: Option<&SyncIngestContext>) -> Result<()> {
        let last_before = if projector_legacy_mode() {
            None
        } else {
            self.last_event_seq_for_session(&e.session_id)?
        };
        let payload = serde_json::to_string(&e.payload)?;
        self.conn.execute(
            "INSERT INTO events (
                session_id, seq, ts_ms, ts_exact, kind, source, tool, tool_call_id,
                tokens_in, tokens_out, reasoning_tokens, cost_usd_e6, payload,
                stop_reason, latency_ms, ttft_ms, retry_count,
                context_used_tokens, context_max_tokens,
                cache_creation_tokens, cache_read_tokens, system_prompt_tokens
             ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22
             )
             ON CONFLICT(session_id, seq) DO UPDATE SET
                ts_ms = excluded.ts_ms,
                ts_exact = excluded.ts_exact,
                kind = excluded.kind,
                source = excluded.source,
                tool = excluded.tool,
                tool_call_id = excluded.tool_call_id,
                tokens_in = excluded.tokens_in,
                tokens_out = excluded.tokens_out,
                reasoning_tokens = excluded.reasoning_tokens,
                cost_usd_e6 = excluded.cost_usd_e6,
                payload = excluded.payload,
                stop_reason = excluded.stop_reason,
                latency_ms = excluded.latency_ms,
                ttft_ms = excluded.ttft_ms,
                retry_count = excluded.retry_count,
                context_used_tokens = excluded.context_used_tokens,
                context_max_tokens = excluded.context_max_tokens,
                cache_creation_tokens = excluded.cache_creation_tokens,
                cache_read_tokens = excluded.cache_read_tokens,
                system_prompt_tokens = excluded.system_prompt_tokens",
            params![
                e.session_id,
                e.seq as i64,
                e.ts_ms as i64,
                bool_to_i64(e.ts_exact),
                format!("{:?}", e.kind),
                format!("{:?}", e.source),
                e.tool,
                e.tool_call_id,
                e.tokens_in.map(|v| v as i64),
                e.tokens_out.map(|v| v as i64),
                e.reasoning_tokens.map(|v| v as i64),
                e.cost_usd_e6,
                payload,
                e.stop_reason,
                e.latency_ms.map(|v| v as i64),
                e.ttft_ms.map(|v| v as i64),
                e.retry_count.map(|v| v as i64),
                e.context_used_tokens.map(|v| v as i64),
                e.context_max_tokens.map(|v| v as i64),
                e.cache_creation_tokens.map(|v| v as i64),
                e.cache_read_tokens.map(|v| v as i64),
                e.system_prompt_tokens.map(|v| v as i64),
            ],
        )?;
        if self.conn.changes() == 0 {
            return Ok(());
        }
        self.append_hot_event(e)?;
        if projector_legacy_mode() {
            index_event_derived(&self.conn, e)?;
            rebuild_tool_spans_for_session(&self.conn, &e.session_id)?;
            self.invalidate_span_tree_cache();
        } else if last_before.is_some_and(|last| e.seq <= last) {
            self.replay_projector_session(&e.session_id)?;
        } else {
            let deltas = self.projector.borrow_mut().apply(e);
            self.apply_projector_events(&deltas)?;
            let expired = self
                .projector
                .borrow_mut()
                .flush_expired(e.ts_ms, DEFAULT_ORPHAN_TTL_MS);
            self.apply_projector_events(&expired)?;
            if is_stop_event(e) {
                let flushed = self
                    .projector
                    .borrow_mut()
                    .flush_session(&e.session_id, e.ts_ms);
                self.apply_projector_events(&flushed)?;
            }
            self.invalidate_span_tree_cache();
        }
        self.append_search_event(e);
        let Some(ctx) = ctx else {
            return Ok(());
        };
        let sync = &ctx.sync;
        if sync.endpoint.is_empty() || sync.team_token.is_empty() || sync.team_id.is_empty() {
            return Ok(());
        }
        let Some(salt) = try_team_salt(sync) else {
            tracing::warn!(
                "sync outbox skipped: set sync.team_salt_hex (64 hex chars) in ~/.kaizen/config.toml"
            );
            return Ok(());
        };
        if sync.sample_rate < 1.0 {
            let u: f64 = rand::random();
            if u > sync.sample_rate {
                return Ok(());
            }
        }
        let Some(session) = self.get_session(&e.session_id)? else {
            tracing::warn!(session_id = %e.session_id, "sync outbox skipped: session not in DB");
            return Ok(());
        };
        let mut outbound = outbound_event_from_row(e, &session, &salt);
        redact_payload(&mut outbound.payload, ctx.workspace_root(), &salt);
        let row = serde_json::to_string(&outbound)?;
        self.outbox()?.append(&e.session_id, "events", &row)?;
        enqueue_tool_spans_for_session(self, &e.session_id, ctx)?;
        Ok(())
    }

    fn append_hot_event(&self, e: &Event) -> Result<()> {
        if std::env::var("KAIZEN_HOT_LOG").as_deref() == Ok("0") {
            return Ok(());
        }
        let mut slot = self.hot_log.borrow_mut();
        if slot.is_none() {
            *slot = Some(HotLog::open(&self.root)?);
        }
        if let Some(log) = slot.as_mut() {
            log.append(e)?;
        }
        Ok(())
    }

    fn append_search_event(&self, e: &Event) {
        if let Err(err) = self.try_append_search_event(e) {
            tracing::warn!(session_id = %e.session_id, seq = e.seq, "search index skipped: {err:#}");
            let _ = self.sync_state_set_u64(SYNC_STATE_SEARCH_DIRTY_MS, now_ms());
        }
    }

    fn try_append_search_event(&self, e: &Event) -> Result<()> {
        let Some(session) = self.get_session(&e.session_id)? else {
            return Ok(());
        };
        let workspace = PathBuf::from(&session.workspace);
        let cfg = crate::core::config::load(&workspace).unwrap_or_default();
        let salt = try_team_salt(&cfg.sync).unwrap_or([0; 32]);
        let Some(doc) = crate::search::extract_doc(e, &session, &workspace, &salt) else {
            return Ok(());
        };
        let mut slot = self.search_writer.borrow_mut();
        if slot.is_none() {
            *slot = Some(crate::search::PendingWriter::open(&self.root)?);
        }
        slot.as_mut().expect("writer").add(&doc)
    }

    pub fn flush_search(&self) -> Result<()> {
        if let Some(writer) = self.search_writer.borrow_mut().as_mut() {
            writer.commit()?;
        }
        Ok(())
    }

    fn outbox(&self) -> Result<Outbox> {
        Outbox::open(&self.root)
    }

    pub fn flush_projector_session(&self, session_id: &str, now_ms: u64) -> Result<()> {
        if projector_legacy_mode() {
            rebuild_tool_spans_for_session(&self.conn, session_id)?;
            self.invalidate_span_tree_cache();
            return Ok(());
        }
        let deltas = self
            .projector
            .borrow_mut()
            .flush_session(session_id, now_ms);
        if self.apply_projector_events(&deltas)? {
            self.invalidate_span_tree_cache();
        }
        Ok(())
    }

    fn replay_projector_session(&self, session_id: &str) -> Result<()> {
        clear_session_spans(&self.conn, session_id)?;
        self.projector.borrow_mut().reset_session(session_id);
        let events = self.list_events_for_session(session_id)?;
        let mut changed = false;
        for event in &events {
            let deltas = self.projector.borrow_mut().apply(event);
            changed |= self.apply_projector_events(&deltas)?;
        }
        if self
            .get_session(session_id)?
            .is_some_and(|session| session.status == SessionStatus::Done)
        {
            let now_ms = events.last().map(|event| event.ts_ms).unwrap_or(0);
            let deltas = self
                .projector
                .borrow_mut()
                .flush_session(session_id, now_ms);
            changed |= self.apply_projector_events(&deltas)?;
        }
        if changed {
            self.invalidate_span_tree_cache();
        }
        Ok(())
    }

    fn apply_projector_events(&self, deltas: &[ProjectorEvent]) -> Result<bool> {
        let mut changed = false;
        for delta in deltas {
            match delta {
                ProjectorEvent::SpanClosed(span, sample) => {
                    upsert_tool_span_record(&self.conn, span)?;
                    tracing::debug!(
                        session_id = %sample.session_id,
                        span_id = %sample.span_id,
                        tool = ?sample.tool,
                        lead_time_ms = ?sample.lead_time_ms,
                        tokens_in = ?sample.tokens_in,
                        tokens_out = ?sample.tokens_out,
                        reasoning_tokens = ?sample.reasoning_tokens,
                        cost_usd_e6 = ?sample.cost_usd_e6,
                        paths = ?sample.paths,
                        "tool span closed"
                    );
                    changed = true;
                }
                ProjectorEvent::SpanPatched(span) => {
                    upsert_tool_span_record(&self.conn, span)?;
                    changed = true;
                }
                ProjectorEvent::FileTouched { session, path } => {
                    self.conn.execute(
                        "INSERT OR IGNORE INTO files_touched (session_id, path) VALUES (?1, ?2)",
                        params![session, path],
                    )?;
                    changed = true;
                }
                ProjectorEvent::SkillUsed { session, skill } => {
                    self.conn.execute(
                        "INSERT OR IGNORE INTO skills_used (session_id, skill) VALUES (?1, ?2)",
                        params![session, skill],
                    )?;
                    changed = true;
                }
                ProjectorEvent::RuleUsed { session, rule } => {
                    self.conn.execute(
                        "INSERT OR IGNORE INTO rules_used (session_id, rule) VALUES (?1, ?2)",
                        params![session, rule],
                    )?;
                    changed = true;
                }
            }
        }
        Ok(changed)
    }

    pub fn list_outbox_pending(&self, limit: usize) -> Result<Vec<(i64, String, String)>> {
        let rows = self.outbox()?.list_pending(limit)?;
        if !rows.is_empty() {
            return Ok(rows);
        }
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, payload FROM sync_outbox WHERE sent = 0 ORDER BY id ASC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn mark_outbox_sent(&self, ids: &[i64]) -> Result<()> {
        self.outbox()?.delete_ids(ids)?;
        for id in ids {
            self.conn
                .execute("UPDATE sync_outbox SET sent = 1 WHERE id = ?1", params![id])?;
        }
        Ok(())
    }

    pub fn replace_outbox_rows(
        &self,
        owner_id: &str,
        kind: &str,
        payloads: &[String],
    ) -> Result<()> {
        self.outbox()?.replace(owner_id, kind, payloads)?;
        self.conn.execute(
            "DELETE FROM sync_outbox WHERE session_id = ?1 AND kind = ?2 AND sent = 0",
            params![owner_id, kind],
        )?;
        for payload in payloads {
            self.conn.execute(
                "INSERT INTO sync_outbox (session_id, kind, payload, sent) VALUES (?1, ?2, ?3, 0)",
                params![owner_id, kind, payload],
            )?;
        }
        Ok(())
    }

    pub fn outbox_pending_count(&self) -> Result<u64> {
        let redb = self.outbox()?.pending_count()?;
        if redb > 0 {
            return Ok(redb);
        }
        let c: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM sync_outbox WHERE sent = 0", [], |r| {
                    r.get(0)
                })?;
        Ok(c as u64)
    }

    pub fn set_sync_state_ok(&self) -> Result<()> {
        let now = now_ms().to_string();
        self.conn.execute(
            "INSERT INTO sync_state (k, v) VALUES ('last_success_ms', ?1)
             ON CONFLICT(k) DO UPDATE SET v = excluded.v",
            params![now],
        )?;
        self.conn.execute(
            "INSERT INTO sync_state (k, v) VALUES ('consecutive_failures', '0')
             ON CONFLICT(k) DO UPDATE SET v = excluded.v",
            [],
        )?;
        self.conn
            .execute("DELETE FROM sync_state WHERE k = 'last_error'", [])?;
        Ok(())
    }

    pub fn set_sync_state_error(&self, msg: &str) -> Result<()> {
        let prev: i64 = self
            .conn
            .query_row(
                "SELECT v FROM sync_state WHERE k = 'consecutive_failures'",
                [],
                |r| {
                    let s: String = r.get(0)?;
                    Ok(s.parse::<i64>().unwrap_or(0))
                },
            )
            .optional()?
            .unwrap_or(0);
        let next = prev.saturating_add(1);
        self.conn.execute(
            "INSERT INTO sync_state (k, v) VALUES ('last_error', ?1)
             ON CONFLICT(k) DO UPDATE SET v = excluded.v",
            params![msg],
        )?;
        self.conn.execute(
            "INSERT INTO sync_state (k, v) VALUES ('consecutive_failures', ?1)
             ON CONFLICT(k) DO UPDATE SET v = excluded.v",
            params![next.to_string()],
        )?;
        Ok(())
    }

    pub fn sync_status(&self) -> Result<SyncStatusSnapshot> {
        let pending_outbox = self.outbox_pending_count()?;
        let last_success_ms = self
            .conn
            .query_row(
                "SELECT v FROM sync_state WHERE k = 'last_success_ms'",
                [],
                |r| r.get::<_, String>(0),
            )
            .optional()?
            .and_then(|s| s.parse().ok());
        let last_error = self
            .conn
            .query_row("SELECT v FROM sync_state WHERE k = 'last_error'", [], |r| {
                r.get::<_, String>(0)
            })
            .optional()?;
        let consecutive_failures = self
            .conn
            .query_row(
                "SELECT v FROM sync_state WHERE k = 'consecutive_failures'",
                [],
                |r| r.get::<_, String>(0),
            )
            .optional()?
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        Ok(SyncStatusSnapshot {
            pending_outbox,
            last_success_ms,
            last_error,
            consecutive_failures,
        })
    }

    pub fn sync_state_get_u64(&self, key: &str) -> Result<Option<u64>> {
        let row: Option<String> = self
            .conn
            .query_row("SELECT v FROM sync_state WHERE k = ?1", params![key], |r| {
                r.get::<_, String>(0)
            })
            .optional()?;
        Ok(row.and_then(|s| s.parse().ok()))
    }

    pub fn sync_state_set_u64(&self, key: &str, v: u64) -> Result<()> {
        self.conn.execute(
            "INSERT INTO sync_state (k, v) VALUES (?1, ?2)
             ON CONFLICT(k) DO UPDATE SET v = excluded.v",
            params![key, v.to_string()],
        )?;
        Ok(())
    }

    /// Delete sessions with `started_at_ms` strictly before `cutoff_ms` and all dependent rows.
    pub fn prune_sessions_started_before(&self, cutoff_ms: i64) -> Result<PruneStats> {
        let tx = rusqlite::Transaction::new_unchecked(&self.conn, TransactionBehavior::Deferred)?;
        let old_ids = old_session_ids(&tx, cutoff_ms)?;
        let sessions_to_remove: i64 = tx.query_row(
            "SELECT COUNT(*) FROM sessions WHERE started_at_ms < ?1",
            params![cutoff_ms],
            |r| r.get(0),
        )?;
        let events_to_remove: i64 = tx.query_row(
            "SELECT COUNT(*) FROM events WHERE session_id IN \
             (SELECT id FROM sessions WHERE started_at_ms < ?1)",
            params![cutoff_ms],
            |r| r.get(0),
        )?;

        let sub_old_sessions = "SELECT id FROM sessions WHERE started_at_ms < ?1";
        tx.execute(
            &format!(
                "DELETE FROM tool_span_paths WHERE span_id IN \
                 (SELECT span_id FROM tool_spans WHERE session_id IN ({sub_old_sessions}))"
            ),
            params![cutoff_ms],
        )?;
        tx.execute(
            &format!("DELETE FROM tool_spans WHERE session_id IN ({sub_old_sessions})"),
            params![cutoff_ms],
        )?;
        tx.execute(
            &format!("DELETE FROM events WHERE session_id IN ({sub_old_sessions})"),
            params![cutoff_ms],
        )?;
        tx.execute(
            &format!("DELETE FROM files_touched WHERE session_id IN ({sub_old_sessions})"),
            params![cutoff_ms],
        )?;
        tx.execute(
            &format!("DELETE FROM skills_used WHERE session_id IN ({sub_old_sessions})"),
            params![cutoff_ms],
        )?;
        tx.execute(
            &format!("DELETE FROM rules_used WHERE session_id IN ({sub_old_sessions})"),
            params![cutoff_ms],
        )?;
        tx.execute(
            &format!("DELETE FROM sync_outbox WHERE session_id IN ({sub_old_sessions})"),
            params![cutoff_ms],
        )?;
        tx.execute(
            &format!("DELETE FROM session_repo_binding WHERE session_id IN ({sub_old_sessions})"),
            params![cutoff_ms],
        )?;
        tx.execute(
            &format!("DELETE FROM experiment_tags WHERE session_id IN ({sub_old_sessions})"),
            params![cutoff_ms],
        )?;
        tx.execute(
            &format!("DELETE FROM session_outcomes WHERE session_id IN ({sub_old_sessions})"),
            params![cutoff_ms],
        )?;
        tx.execute(
            &format!("DELETE FROM session_samples WHERE session_id IN ({sub_old_sessions})"),
            params![cutoff_ms],
        )?;
        tx.execute(
            "DELETE FROM sessions WHERE started_at_ms < ?1",
            params![cutoff_ms],
        )?;
        tx.commit()?;
        if let Some(mut writer) = self.search_writer.borrow_mut().take() {
            let _ = writer.commit();
        }
        if let Err(err) = crate::search::delete_sessions(&self.root, &old_ids) {
            tracing::warn!("search prune skipped: {err:#}");
            let _ = self.sync_state_set_u64(SYNC_STATE_SEARCH_DIRTY_MS, now_ms());
        }
        self.invalidate_span_tree_cache();
        Ok(PruneStats {
            sessions_removed: sessions_to_remove as u64,
            events_removed: events_to_remove as u64,
        })
    }

    /// Reclaim file space after large deletes (exclusive lock; can be slow).
    pub fn vacuum(&self) -> Result<()> {
        self.conn.execute_batch("VACUUM;").context("VACUUM")?;
        Ok(())
    }

    pub fn list_sessions(&self, workspace: &str) -> Result<Vec<SessionRecord>> {
        Ok(self
            .list_sessions_page(workspace, 0, i64::MAX as usize, SessionFilter::default())?
            .rows)
    }

    pub fn list_sessions_page(
        &self,
        workspace: &str,
        offset: usize,
        limit: usize,
        filter: SessionFilter,
    ) -> Result<SessionPage> {
        let (where_sql, args) = session_filter_sql(workspace, &filter);
        let total = self.query_session_page_count(&where_sql, &args)?;
        let rows = self.query_session_page_rows(&where_sql, &args, offset, limit)?;
        let next = offset.saturating_add(rows.len());
        Ok(SessionPage {
            rows,
            total,
            next_offset: (next < total).then_some(next),
        })
    }

    fn query_session_page_count(&self, where_sql: &str, args: &[Value]) -> Result<usize> {
        let sql = format!("SELECT COUNT(*) FROM sessions {where_sql}");
        let total: i64 = self
            .conn
            .query_row(&sql, params_from_iter(args.iter()), |r| r.get(0))?;
        Ok(total as usize)
    }

    fn query_session_page_rows(
        &self,
        where_sql: &str,
        args: &[Value],
        offset: usize,
        limit: usize,
    ) -> Result<Vec<SessionRecord>> {
        let sql = format!(
            "{SESSION_SELECT} {where_sql} ORDER BY started_at_ms DESC, id ASC LIMIT ? OFFSET ?"
        );
        let mut values = args.to_vec();
        values.push(Value::Integer(limit.min(i64::MAX as usize) as i64));
        values.push(Value::Integer(offset.min(i64::MAX as usize) as i64));
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(values.iter()), session_row)?;
        rows.map(|r| r.map_err(anyhow::Error::from)).collect()
    }

    pub fn list_sessions_started_after(
        &self,
        workspace: &str,
        after_started_at_ms: u64,
    ) -> Result<Vec<SessionRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, agent, model, workspace, started_at_ms, ended_at_ms, status, trace_path,
                    start_commit, end_commit, branch, dirty_start, dirty_end, repo_binding_source,
                    prompt_fingerprint, parent_session_id, agent_version, os, arch,
                    repo_file_count, repo_total_loc
             FROM sessions
             WHERE workspace = ?1 AND started_at_ms > ?2
             ORDER BY started_at_ms DESC, id ASC",
        )?;
        let rows = stmt.query_map(params![workspace, after_started_at_ms as i64], session_row)?;
        rows.map(|r| r.map_err(anyhow::Error::from)).collect()
    }

    pub fn session_statuses(&self, ids: &[String]) -> Result<Vec<SessionStatusRow>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql =
            format!("SELECT id, status, ended_at_ms FROM sessions WHERE id IN ({placeholders})");
        let mut stmt = self.conn.prepare(&sql)?;
        let params: Vec<&dyn rusqlite::ToSql> =
            ids.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
        let rows = stmt.query_map(params.as_slice(), |r| {
            let status: String = r.get(1)?;
            Ok(SessionStatusRow {
                id: r.get(0)?,
                status: status_from_str(&status),
                ended_at_ms: r.get::<_, Option<i64>>(2)?.map(|v| v as u64),
            })
        })?;
        rows.map(|r| r.map_err(anyhow::Error::from)).collect()
    }

    fn running_session_ids(&self) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id FROM sessions WHERE status != 'Done' ORDER BY started_at_ms ASC")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        rows.map(|r| r.map_err(anyhow::Error::from)).collect()
    }

    pub fn summary_stats(&self, workspace: &str) -> Result<SummaryStats> {
        let session_count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM sessions WHERE workspace = ?1",
            params![workspace],
            |r| r.get(0),
        )?;

        let total_cost: i64 = self.conn.query_row(
            "SELECT COALESCE(SUM(e.cost_usd_e6), 0) FROM events e
             JOIN sessions s ON s.id = e.session_id WHERE s.workspace = ?1",
            params![workspace],
            |r| r.get(0),
        )?;

        let mut stmt = self.conn.prepare(
            "SELECT agent, COUNT(*) FROM sessions WHERE workspace = ?1 GROUP BY agent ORDER BY COUNT(*) DESC",
        )?;
        let by_agent: Vec<(String, u64)> = stmt
            .query_map(params![workspace], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? as u64))
            })?
            .filter_map(|r| r.ok())
            .collect();

        let mut stmt = self.conn.prepare(
            "SELECT COALESCE(model, 'unknown'), COUNT(*) FROM sessions WHERE workspace = ?1 GROUP BY model ORDER BY COUNT(*) DESC",
        )?;
        let by_model: Vec<(String, u64)> = stmt
            .query_map(params![workspace], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? as u64))
            })?
            .filter_map(|r| r.ok())
            .collect();

        let mut stmt = self.conn.prepare(
            "SELECT tool, COUNT(*) FROM events e JOIN sessions s ON s.id = e.session_id
             WHERE s.workspace = ?1 AND tool IS NOT NULL
             GROUP BY tool ORDER BY COUNT(*) DESC LIMIT 10",
        )?;
        let top_tools: Vec<(String, u64)> = stmt
            .query_map(params![workspace], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? as u64))
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(SummaryStats {
            session_count: session_count as u64,
            total_cost_usd_e6: total_cost,
            by_agent,
            by_model,
            top_tools,
        })
    }

    pub fn list_events_for_session(&self, session_id: &str) -> Result<Vec<Event>> {
        self.list_events_page(session_id, 0, i64::MAX as usize)
    }

    pub fn get_event(&self, session_id: &str, seq: u64) -> Result<Option<Event>> {
        let mut stmt = self.conn.prepare(
            "SELECT session_id, seq, ts_ms, COALESCE(ts_exact, 0), kind, source, tool, tool_call_id,
                    tokens_in, tokens_out, reasoning_tokens, cost_usd_e6, payload,
                    stop_reason, latency_ms, ttft_ms, retry_count,
                    context_used_tokens, context_max_tokens,
                    cache_creation_tokens, cache_read_tokens, system_prompt_tokens
             FROM events WHERE session_id = ?1 AND seq = ?2",
        )?;
        stmt.query_row(params![session_id, seq as i64], event_row)
            .optional()
            .map_err(Into::into)
    }

    pub fn search_tool_events(
        &self,
        workspace: &str,
        tool: &str,
        since_ms: Option<u64>,
        agent: Option<&str>,
        limit: usize,
    ) -> Result<Vec<(String, Event)>> {
        let mut stmt = self.conn.prepare(
            "SELECT e.session_id, e.seq, e.ts_ms, COALESCE(e.ts_exact, 0), e.kind, e.source, e.tool, e.tool_call_id,
                    e.tokens_in, e.tokens_out, e.reasoning_tokens, e.cost_usd_e6, e.payload,
                    e.stop_reason, e.latency_ms, e.ttft_ms, e.retry_count,
                    e.context_used_tokens, e.context_max_tokens,
                    e.cache_creation_tokens, e.cache_read_tokens, e.system_prompt_tokens,
                    s.agent
             FROM events e JOIN sessions s ON s.id = e.session_id
             WHERE e.tool = ?2
               AND (s.workspace = ?1 OR NOT EXISTS (SELECT 1 FROM sessions WHERE workspace = ?1))
               AND (?3 IS NULL OR e.ts_ms >= ?3)
               AND (?4 IS NULL OR s.agent = ?4)
             ORDER BY e.ts_ms DESC, e.session_id ASC, e.seq ASC
             LIMIT ?5",
        )?;
        let since = since_ms.map(|v| v as i64);
        let rows = stmt.query_map(
            params![workspace, tool, since, agent, limit as i64],
            search_tool_event_row,
        )?;
        rows.map(|r| r.map_err(anyhow::Error::from)).collect()
    }

    pub fn workspace_events(&self, workspace: &str) -> Result<Vec<(SessionRecord, Event)>> {
        let mut out = Vec::new();
        for session in self.list_sessions(workspace)? {
            for event in self.list_events_for_session(&session.id)? {
                out.push((session.clone(), event));
            }
        }
        out.sort_by(|a, b| {
            (a.1.ts_ms, &a.1.session_id, a.1.seq).cmp(&(b.1.ts_ms, &b.1.session_id, b.1.seq))
        });
        Ok(out)
    }

    pub fn list_events_page(
        &self,
        session_id: &str,
        after_seq: u64,
        limit: usize,
    ) -> Result<Vec<Event>> {
        let mut stmt = self.conn.prepare(
            "SELECT session_id, seq, ts_ms, COALESCE(ts_exact, 0), kind, source, tool, tool_call_id,
                    tokens_in, tokens_out, reasoning_tokens, cost_usd_e6, payload,
                    stop_reason, latency_ms, ttft_ms, retry_count,
                    context_used_tokens, context_max_tokens,
                    cache_creation_tokens, cache_read_tokens, system_prompt_tokens
             FROM events
             WHERE session_id = ?1 AND seq >= ?2
             ORDER BY seq ASC LIMIT ?3",
        )?;
        let rows = stmt.query_map(
            params![
                session_id,
                after_seq as i64,
                limit.min(i64::MAX as usize) as i64
            ],
            event_row,
        )?;
        let mut events = Vec::new();
        for row in rows {
            events.push(row?);
        }
        Ok(events)
    }

    /// Update only status for existing session.
    pub fn update_session_status(&self, id: &str, status: SessionStatus) -> Result<()> {
        self.conn.execute(
            "UPDATE sessions SET status = ?1 WHERE id = ?2",
            params![format!("{:?}", status), id],
        )?;
        Ok(())
    }

    /// Workspace activity dashboard — feeds `cmd_insights`.
    pub fn insights(&self, workspace: &str) -> Result<InsightsStats> {
        let (total_cost_usd_e6, sessions_with_cost) = cost_stats(&self.conn, workspace)?;
        Ok(InsightsStats {
            total_sessions: count_q(
                &self.conn,
                "SELECT COUNT(*) FROM sessions WHERE workspace=?1",
                workspace,
            )?,
            running_sessions: count_q(
                &self.conn,
                "SELECT COUNT(*) FROM sessions WHERE workspace=?1 AND status='Running'",
                workspace,
            )?,
            total_events: count_q(
                &self.conn,
                "SELECT COUNT(*) FROM events e JOIN sessions s ON s.id=e.session_id WHERE s.workspace=?1",
                workspace,
            )?,
            sessions_by_day: sessions_by_day_7(&self.conn, workspace, now_ms())?,
            recent: recent_sessions_3(&self.conn, workspace)?,
            top_tools: top_tools_5(&self.conn, workspace)?,
            total_cost_usd_e6,
            sessions_with_cost,
        })
    }

    /// Events in `[start_ms, end_ms]` for a workspace, with session metadata per row.
    pub fn retro_events_in_window(
        &self,
        workspace: &str,
        start_ms: u64,
        end_ms: u64,
    ) -> Result<Vec<(SessionRecord, Event)>> {
        let mut stmt = self.conn.prepare(
            "SELECT e.session_id, e.seq, e.ts_ms, COALESCE(e.ts_exact, 0), e.kind, e.source, e.tool, e.tool_call_id,
                    e.tokens_in, e.tokens_out, e.reasoning_tokens, e.cost_usd_e6, e.payload,
                    s.id, s.agent, s.model, s.workspace, s.started_at_ms, s.ended_at_ms, s.status, s.trace_path,
                    s.start_commit, s.end_commit, s.branch, s.dirty_start, s.dirty_end, s.repo_binding_source,
                    s.prompt_fingerprint, s.parent_session_id, s.agent_version, s.os, s.arch,
                    s.repo_file_count, s.repo_total_loc,
                    e.stop_reason, e.latency_ms, e.ttft_ms, e.retry_count,
                    e.context_used_tokens, e.context_max_tokens,
                    e.cache_creation_tokens, e.cache_read_tokens, e.system_prompt_tokens
             FROM events e
             JOIN sessions s ON s.id = e.session_id
             WHERE s.workspace = ?1
               AND (
                 (e.ts_ms >= ?2 AND e.ts_ms <= ?3)
                 OR (e.ts_ms < ?4 AND s.started_at_ms >= ?2 AND s.started_at_ms <= ?3)
               )
             ORDER BY e.ts_ms ASC, e.session_id ASC, e.seq ASC",
        )?;
        let rows = stmt.query_map(
            params![
                workspace,
                start_ms as i64,
                end_ms as i64,
                SYNTHETIC_TS_CEILING_MS,
            ],
            |row| {
                let payload_str: String = row.get(12)?;
                let status_str: String = row.get(19)?;
                Ok((
                    SessionRecord {
                        id: row.get(13)?,
                        agent: row.get(14)?,
                        model: row.get(15)?,
                        workspace: row.get(16)?,
                        started_at_ms: row.get::<_, i64>(17)? as u64,
                        ended_at_ms: row.get::<_, Option<i64>>(18)?.map(|v| v as u64),
                        status: status_from_str(&status_str),
                        trace_path: row.get(20)?,
                        start_commit: row.get(21)?,
                        end_commit: row.get(22)?,
                        branch: row.get(23)?,
                        dirty_start: row.get::<_, Option<i64>>(24)?.map(i64_to_bool),
                        dirty_end: row.get::<_, Option<i64>>(25)?.map(i64_to_bool),
                        repo_binding_source: empty_to_none(row.get::<_, String>(26)?),
                        prompt_fingerprint: row.get(27)?,
                        parent_session_id: row.get(28)?,
                        agent_version: row.get(29)?,
                        os: row.get(30)?,
                        arch: row.get(31)?,
                        repo_file_count: row.get::<_, Option<i64>>(32)?.map(|v| v as u32),
                        repo_total_loc: row.get::<_, Option<i64>>(33)?.map(|v| v as u64),
                    },
                    Event {
                        session_id: row.get(0)?,
                        seq: row.get::<_, i64>(1)? as u64,
                        ts_ms: row.get::<_, i64>(2)? as u64,
                        ts_exact: row.get::<_, i64>(3)? != 0,
                        kind: kind_from_str(&row.get::<_, String>(4)?),
                        source: source_from_str(&row.get::<_, String>(5)?),
                        tool: row.get(6)?,
                        tool_call_id: row.get(7)?,
                        tokens_in: row.get::<_, Option<i64>>(8)?.map(|v| v as u32),
                        tokens_out: row.get::<_, Option<i64>>(9)?.map(|v| v as u32),
                        reasoning_tokens: row.get::<_, Option<i64>>(10)?.map(|v| v as u32),
                        cost_usd_e6: row.get(11)?,
                        payload: serde_json::from_str(&payload_str)
                            .unwrap_or(serde_json::Value::Null),
                        stop_reason: row.get(34)?,
                        latency_ms: row.get::<_, Option<i64>>(35)?.map(|v| v as u32),
                        ttft_ms: row.get::<_, Option<i64>>(36)?.map(|v| v as u32),
                        retry_count: row.get::<_, Option<i64>>(37)?.map(|v| v as u16),
                        context_used_tokens: row.get::<_, Option<i64>>(38)?.map(|v| v as u32),
                        context_max_tokens: row.get::<_, Option<i64>>(39)?.map(|v| v as u32),
                        cache_creation_tokens: row.get::<_, Option<i64>>(40)?.map(|v| v as u32),
                        cache_read_tokens: row.get::<_, Option<i64>>(41)?.map(|v| v as u32),
                        system_prompt_tokens: row.get::<_, Option<i64>>(42)?.map(|v| v as u32),
                    },
                ))
            },
        )?;

        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn experiment_metric_values_in_window(
        &self,
        workspace: &str,
        start_ms: u64,
        end_ms: u64,
        metric: crate::experiment::types::Metric,
    ) -> Result<Vec<(SessionRecord, f64)>> {
        use crate::experiment::types::Metric;
        let session_cols = "s.id, s.agent, s.model, s.workspace, s.started_at_ms, s.ended_at_ms,
            s.status, s.trace_path, s.start_commit, s.end_commit, s.branch, s.dirty_start,
            s.dirty_end, s.repo_binding_source, s.prompt_fingerprint, s.parent_session_id,
            s.agent_version, s.os, s.arch, s.repo_file_count, s.repo_total_loc";
        let window = "s.workspace = ?1 AND ((e.ts_ms >= ?2 AND e.ts_ms <= ?3)
            OR (e.ts_ms < ?4 AND s.started_at_ms >= ?2 AND s.started_at_ms <= ?3))";
        let sql = match metric {
            Metric::TokensPerSession => format!(
                "SELECT {session_cols},
                    SUM(COALESCE(e.tokens_in,0)+COALESCE(e.tokens_out,0)+COALESCE(e.reasoning_tokens,0)) AS value
                 FROM sessions s JOIN events e ON e.session_id = s.id
                 WHERE {window}
                 GROUP BY s.id"
            ),
            Metric::CostPerSession => format!(
                "SELECT {session_cols}, SUM(COALESCE(e.cost_usd_e6,0)) / 1000000.0 AS value
                 FROM sessions s JOIN events e ON e.session_id = s.id
                 WHERE {window}
                 GROUP BY s.id"
            ),
            Metric::SuccessRate => format!(
                "SELECT {session_cols},
                    CASE WHEN SUM(CASE WHEN e.kind='Error' THEN 1 ELSE 0 END) > 0 THEN 0.0 ELSE 1.0 END AS value
                 FROM sessions s JOIN events e ON e.session_id = s.id
                 WHERE {window}
                 GROUP BY s.id"
            ),
            Metric::DurationMinutes => format!(
                "SELECT {session_cols},
                    (s.ended_at_ms - s.started_at_ms) / 60000.0 AS value
                 FROM sessions s
                 WHERE s.workspace = ?1
                   AND s.ended_at_ms IS NOT NULL
                   AND EXISTS (
                     SELECT 1 FROM events e
                     WHERE e.session_id = s.id
                       AND ((e.ts_ms >= ?2 AND e.ts_ms <= ?3)
                         OR (e.ts_ms < ?4 AND s.started_at_ms >= ?2 AND s.started_at_ms <= ?3))
                   )"
            ),
            Metric::FilesPerSession => format!(
                "SELECT {session_cols}, COUNT(DISTINCT ft.path) AS value
                 FROM sessions s
                 JOIN events e ON e.session_id = s.id
                 LEFT JOIN files_touched ft ON ft.session_id = s.id
                 WHERE {window}
                 GROUP BY s.id"
            ),
            Metric::SuccessRateByPrompt => format!(
                "SELECT {session_cols},
                    1.0 - (MIN(
                      SUM(CASE WHEN e.kind='Error' THEN 1 ELSE 0 END),
                      SUM(CASE WHEN e.kind='Message' THEN 1 ELSE 0 END)
                    ) * 1.0 / SUM(CASE WHEN e.kind='Message' THEN 1 ELSE 0 END)) AS value
                 FROM sessions s JOIN events e ON e.session_id = s.id
                 WHERE {window}
                 GROUP BY s.id
                 HAVING SUM(CASE WHEN e.kind='Message' THEN 1 ELSE 0 END) > 0"
            ),
            Metric::CostByPrompt => format!(
                "SELECT {session_cols},
                    SUM(COALESCE(e.cost_usd_e6,0)) / 1000000.0 /
                    SUM(CASE WHEN e.kind='Message' THEN 1 ELSE 0 END) AS value
                 FROM sessions s JOIN events e ON e.session_id = s.id
                 WHERE {window}
                 GROUP BY s.id
                 HAVING SUM(CASE WHEN e.kind='Message' THEN 1 ELSE 0 END) > 0"
            ),
            Metric::ToolLoops => format!(
                "WITH calls AS (
                   SELECT e.session_id, e.tool,
                     LAG(e.tool) OVER (PARTITION BY e.session_id ORDER BY e.ts_ms, e.seq) AS prev_tool
                   FROM events e JOIN sessions s ON s.id = e.session_id
                   WHERE {window} AND e.kind='ToolCall' AND e.tool IS NOT NULL
                 )
                 SELECT {session_cols},
                    SUM(CASE WHEN calls.tool = calls.prev_tool THEN 1 ELSE 0 END) AS value
                 FROM sessions s JOIN calls ON calls.session_id = s.id
                 GROUP BY s.id"
            ),
        };
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(
            params![
                workspace,
                start_ms as i64,
                end_ms as i64,
                SYNTHETIC_TS_CEILING_MS,
            ],
            |row| Ok((session_row(row)?, row.get::<_, f64>(21)?)),
        )?;
        rows.map(|r| r.map_err(anyhow::Error::from)).collect()
    }

    /// Distinct `(session_id, path)` for sessions with activity in the time window.
    pub fn files_touched_in_window(
        &self,
        workspace: &str,
        start_ms: u64,
        end_ms: u64,
    ) -> Result<Vec<(String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT ft.session_id, ft.path
             FROM files_touched ft
             JOIN sessions s ON s.id = ft.session_id
             WHERE s.workspace = ?1
               AND EXISTS (
                 SELECT 1 FROM events e
                 JOIN sessions ss ON ss.id = e.session_id
                 WHERE e.session_id = ft.session_id
                   AND (
                     (e.ts_ms >= ?2 AND e.ts_ms <= ?3)
                     OR (e.ts_ms < ?4 AND ss.started_at_ms >= ?2 AND ss.started_at_ms <= ?3)
                   )
               )
             ORDER BY ft.session_id, ft.path",
        )?;
        let out: Vec<(String, String)> = stmt
            .query_map(
                params![
                    workspace,
                    start_ms as i64,
                    end_ms as i64,
                    SYNTHETIC_TS_CEILING_MS,
                ],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?
            .filter_map(|r| r.ok())
            .collect();
        Ok(out)
    }

    /// Distinct skill slugs referenced in `skills_used` for a workspace since `since_ms`
    /// (any session with an indexed skill row; join events optional — use row existence).
    pub fn skills_used_since(&self, workspace: &str, since_ms: u64) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT su.skill
             FROM skills_used su
             JOIN sessions s ON s.id = su.session_id
             WHERE s.workspace = ?1
               AND EXISTS (
                 SELECT 1 FROM events e
                 JOIN sessions ss ON ss.id = e.session_id
                 WHERE e.session_id = su.session_id
                   AND (e.ts_ms >= ?2 OR (e.ts_ms < ?3 AND ss.started_at_ms >= ?2))
               )
             ORDER BY su.skill",
        )?;
        let out: Vec<String> = stmt
            .query_map(
                params![workspace, since_ms as i64, SYNTHETIC_TS_CEILING_MS],
                |r| r.get::<_, String>(0),
            )?
            .filter_map(|r| r.ok())
            .filter(|s: &String| crate::store::event_index::is_valid_slug(s))
            .collect();
        Ok(out)
    }

    /// Distinct `(session_id, skill)` for sessions with activity in the time window.
    pub fn skills_used_in_window(
        &self,
        workspace: &str,
        start_ms: u64,
        end_ms: u64,
    ) -> Result<Vec<(String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT su.session_id, su.skill
             FROM skills_used su
             JOIN sessions s ON s.id = su.session_id
             WHERE s.workspace = ?1
               AND EXISTS (
                 SELECT 1 FROM events e
                 JOIN sessions ss ON ss.id = e.session_id
                 WHERE e.session_id = su.session_id
                   AND (
                     (e.ts_ms >= ?2 AND e.ts_ms <= ?3)
                     OR (e.ts_ms < ?4 AND ss.started_at_ms >= ?2 AND ss.started_at_ms <= ?3)
                   )
               )
             ORDER BY su.session_id, su.skill",
        )?;
        let out: Vec<(String, String)> = stmt
            .query_map(
                params![
                    workspace,
                    start_ms as i64,
                    end_ms as i64,
                    SYNTHETIC_TS_CEILING_MS,
                ],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            )?
            .filter_map(|r| r.ok())
            .filter(|(_, skill): &(String, String)| crate::store::event_index::is_valid_slug(skill))
            .collect();
        Ok(out)
    }

    /// Distinct rule stems referenced in `rules_used` for a workspace since `since_ms`.
    pub fn rules_used_since(&self, workspace: &str, since_ms: u64) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT ru.rule
             FROM rules_used ru
             JOIN sessions s ON s.id = ru.session_id
             WHERE s.workspace = ?1
               AND EXISTS (
                 SELECT 1 FROM events e
                 JOIN sessions ss ON ss.id = e.session_id
                 WHERE e.session_id = ru.session_id
                   AND (e.ts_ms >= ?2 OR (e.ts_ms < ?3 AND ss.started_at_ms >= ?2))
               )
             ORDER BY ru.rule",
        )?;
        let out: Vec<String> = stmt
            .query_map(
                params![workspace, since_ms as i64, SYNTHETIC_TS_CEILING_MS],
                |r| r.get::<_, String>(0),
            )?
            .filter_map(|r| r.ok())
            .filter(|s: &String| crate::store::event_index::is_valid_slug(s))
            .collect();
        Ok(out)
    }

    /// Distinct `(session_id, rule)` for sessions with activity in the time window.
    pub fn rules_used_in_window(
        &self,
        workspace: &str,
        start_ms: u64,
        end_ms: u64,
    ) -> Result<Vec<(String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT ru.session_id, ru.rule
             FROM rules_used ru
             JOIN sessions s ON s.id = ru.session_id
             WHERE s.workspace = ?1
               AND EXISTS (
                 SELECT 1 FROM events e
                 JOIN sessions ss ON ss.id = e.session_id
                 WHERE e.session_id = ru.session_id
                   AND (
                     (e.ts_ms >= ?2 AND e.ts_ms <= ?3)
                     OR (e.ts_ms < ?4 AND ss.started_at_ms >= ?2 AND ss.started_at_ms <= ?3)
                   )
               )
             ORDER BY ru.session_id, ru.rule",
        )?;
        let out: Vec<(String, String)> = stmt
            .query_map(
                params![
                    workspace,
                    start_ms as i64,
                    end_ms as i64,
                    SYNTHETIC_TS_CEILING_MS,
                ],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            )?
            .filter_map(|r| r.ok())
            .filter(|(_, rule): &(String, String)| crate::store::event_index::is_valid_slug(rule))
            .collect();
        Ok(out)
    }

    /// Sessions with at least one event timestamp falling in `[start_ms, end_ms]` (same rules as retro window).
    pub fn sessions_active_in_window(
        &self,
        workspace: &str,
        start_ms: u64,
        end_ms: u64,
    ) -> Result<HashSet<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT s.id
             FROM sessions s
             WHERE s.workspace = ?1
               AND EXISTS (
                 SELECT 1 FROM events e
                 WHERE e.session_id = s.id
                   AND (
                     (e.ts_ms >= ?2 AND e.ts_ms <= ?3)
                     OR (e.ts_ms < ?4 AND s.started_at_ms >= ?2 AND s.started_at_ms <= ?3)
                   )
               )",
        )?;
        let out: HashSet<String> = stmt
            .query_map(
                params![
                    workspace,
                    start_ms as i64,
                    end_ms as i64,
                    SYNTHETIC_TS_CEILING_MS,
                ],
                |r| r.get(0),
            )?
            .filter_map(|r| r.ok())
            .collect();
        Ok(out)
    }

    /// Per-session sum of `cost_usd_e6` for events in the window (missing costs treated as 0).
    pub fn session_costs_usd_e6_in_window(
        &self,
        workspace: &str,
        start_ms: u64,
        end_ms: u64,
    ) -> Result<HashMap<String, i64>> {
        let mut stmt = self.conn.prepare(
            "SELECT e.session_id, SUM(COALESCE(e.cost_usd_e6, 0))
             FROM events e
             JOIN sessions s ON s.id = e.session_id
             WHERE s.workspace = ?1
               AND (
                 (e.ts_ms >= ?2 AND e.ts_ms <= ?3)
                 OR (e.ts_ms < ?4 AND s.started_at_ms >= ?2 AND s.started_at_ms <= ?3)
               )
             GROUP BY e.session_id",
        )?;
        let rows: Vec<(String, i64)> = stmt
            .query_map(
                params![
                    workspace,
                    start_ms as i64,
                    end_ms as i64,
                    SYNTHETIC_TS_CEILING_MS,
                ],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows.into_iter().collect())
    }

    /// Skill/rule adoption and cost proxy vs workspace average (observed payload references only).
    pub fn guidance_report(
        &self,
        workspace: &str,
        window_start_ms: u64,
        window_end_ms: u64,
        skill_slugs_on_disk: &HashSet<String>,
        rule_slugs_on_disk: &HashSet<String>,
    ) -> Result<GuidanceReport> {
        let active = self.sessions_active_in_window(workspace, window_start_ms, window_end_ms)?;
        let denom = active.len() as u64;
        let costs =
            self.session_costs_usd_e6_in_window(workspace, window_start_ms, window_end_ms)?;

        let workspace_avg_cost_per_session_usd = if denom > 0 {
            let total_e6: i64 = active
                .iter()
                .map(|sid| costs.get(sid).copied().unwrap_or(0))
                .sum();
            Some(total_e6 as f64 / denom as f64 / 1_000_000.0)
        } else {
            None
        };

        let mut skill_sessions: HashMap<String, HashSet<String>> = HashMap::new();
        for (sid, skill) in self.skills_used_in_window(workspace, window_start_ms, window_end_ms)? {
            skill_sessions.entry(skill).or_default().insert(sid);
        }
        let mut rule_sessions: HashMap<String, HashSet<String>> = HashMap::new();
        for (sid, rule) in self.rules_used_in_window(workspace, window_start_ms, window_end_ms)? {
            rule_sessions.entry(rule).or_default().insert(sid);
        }

        let mut rows: Vec<GuidancePerfRow> = Vec::new();

        let mut push_row =
            |kind: GuidanceKind, id: String, sids: &HashSet<String>, on_disk: bool| {
                let sessions = sids.len() as u64;
                let sessions_pct = if denom > 0 {
                    sessions as f64 * 100.0 / denom as f64
                } else {
                    0.0
                };
                let total_cost_usd_e6: i64 = sids
                    .iter()
                    .map(|sid| costs.get(sid).copied().unwrap_or(0))
                    .sum();
                let avg_cost_per_session_usd = if sessions > 0 {
                    Some(total_cost_usd_e6 as f64 / sessions as f64 / 1_000_000.0)
                } else {
                    None
                };
                let vs_workspace_avg_cost_per_session_usd =
                    match (avg_cost_per_session_usd, workspace_avg_cost_per_session_usd) {
                        (Some(avg), Some(w)) => Some(avg - w),
                        _ => None,
                    };
                rows.push(GuidancePerfRow {
                    kind,
                    id,
                    sessions,
                    sessions_pct,
                    total_cost_usd_e6,
                    avg_cost_per_session_usd,
                    vs_workspace_avg_cost_per_session_usd,
                    on_disk,
                });
            };

        let mut seen_skills: HashSet<String> = HashSet::new();
        for (id, sids) in &skill_sessions {
            seen_skills.insert(id.clone());
            push_row(
                GuidanceKind::Skill,
                id.clone(),
                sids,
                skill_slugs_on_disk.contains(id),
            );
        }
        for slug in skill_slugs_on_disk {
            if seen_skills.contains(slug) {
                continue;
            }
            push_row(GuidanceKind::Skill, slug.clone(), &HashSet::new(), true);
        }

        let mut seen_rules: HashSet<String> = HashSet::new();
        for (id, sids) in &rule_sessions {
            seen_rules.insert(id.clone());
            push_row(
                GuidanceKind::Rule,
                id.clone(),
                sids,
                rule_slugs_on_disk.contains(id),
            );
        }
        for slug in rule_slugs_on_disk {
            if seen_rules.contains(slug) {
                continue;
            }
            push_row(GuidanceKind::Rule, slug.clone(), &HashSet::new(), true);
        }

        rows.sort_by(|a, b| {
            b.sessions
                .cmp(&a.sessions)
                .then_with(|| a.kind.cmp(&b.kind))
                .then_with(|| a.id.cmp(&b.id))
        });

        Ok(GuidanceReport {
            workspace: workspace.to_string(),
            window_start_ms,
            window_end_ms,
            sessions_in_window: denom,
            workspace_avg_cost_per_session_usd,
            rows,
        })
    }

    pub fn get_session(&self, id: &str) -> Result<Option<SessionRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, agent, model, workspace, started_at_ms, ended_at_ms, status, trace_path,
                    start_commit, end_commit, branch, dirty_start, dirty_end, repo_binding_source,
                    prompt_fingerprint, parent_session_id, agent_version, os, arch,
                    repo_file_count, repo_total_loc
             FROM sessions WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, Option<i64>>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, Option<String>>(9)?,
                row.get::<_, Option<String>>(10)?,
                row.get::<_, Option<i64>>(11)?,
                row.get::<_, Option<i64>>(12)?,
                row.get::<_, String>(13)?,
                row.get::<_, Option<String>>(14)?,
                row.get::<_, Option<String>>(15)?,
                row.get::<_, Option<String>>(16)?,
                row.get::<_, Option<String>>(17)?,
                row.get::<_, Option<String>>(18)?,
                row.get::<_, Option<i64>>(19)?,
                row.get::<_, Option<i64>>(20)?,
            ))
        })?;

        if let Some(row) = rows.next() {
            let (
                id,
                agent,
                model,
                workspace,
                started,
                ended,
                status_str,
                trace,
                start_commit,
                end_commit,
                branch,
                dirty_start,
                dirty_end,
                source,
                prompt_fingerprint,
                parent_session_id,
                agent_version,
                os,
                arch,
                repo_file_count,
                repo_total_loc,
            ) = row?;
            Ok(Some(SessionRecord {
                id,
                agent,
                model,
                workspace,
                started_at_ms: started as u64,
                ended_at_ms: ended.map(|v| v as u64),
                status: status_from_str(&status_str),
                trace_path: trace,
                start_commit,
                end_commit,
                branch,
                dirty_start: dirty_start.map(i64_to_bool),
                dirty_end: dirty_end.map(i64_to_bool),
                repo_binding_source: empty_to_none(source),
                prompt_fingerprint,
                parent_session_id,
                agent_version,
                os,
                arch,
                repo_file_count: repo_file_count.map(|v| v as u32),
                repo_total_loc: repo_total_loc.map(|v| v as u64),
            }))
        } else {
            Ok(None)
        }
    }

    pub fn latest_repo_snapshot(&self, workspace: &str) -> Result<Option<RepoSnapshotRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, workspace, head_commit, dirty_fingerprint, analyzer_version,
                    indexed_at_ms, dirty, graph_path
             FROM repo_snapshots WHERE workspace = ?1
             ORDER BY indexed_at_ms DESC LIMIT 1",
        )?;
        let mut rows = stmt.query_map(params![workspace], |row| {
            Ok(RepoSnapshotRecord {
                id: row.get(0)?,
                workspace: row.get(1)?,
                head_commit: row.get(2)?,
                dirty_fingerprint: row.get(3)?,
                analyzer_version: row.get(4)?,
                indexed_at_ms: row.get::<_, i64>(5)? as u64,
                dirty: row.get::<_, i64>(6)? != 0,
                graph_path: row.get(7)?,
            })
        })?;
        Ok(rows.next().transpose()?)
    }

    pub fn save_repo_snapshot(
        &self,
        snapshot: &RepoSnapshotRecord,
        facts: &[FileFact],
        edges: &[RepoEdge],
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO repo_snapshots (
                id, workspace, head_commit, dirty_fingerprint, analyzer_version,
                indexed_at_ms, dirty, graph_path
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(id) DO UPDATE SET
                workspace=excluded.workspace,
                head_commit=excluded.head_commit,
                dirty_fingerprint=excluded.dirty_fingerprint,
                analyzer_version=excluded.analyzer_version,
                indexed_at_ms=excluded.indexed_at_ms,
                dirty=excluded.dirty,
                graph_path=excluded.graph_path",
            params![
                snapshot.id,
                snapshot.workspace,
                snapshot.head_commit,
                snapshot.dirty_fingerprint,
                snapshot.analyzer_version,
                snapshot.indexed_at_ms as i64,
                bool_to_i64(snapshot.dirty),
                snapshot.graph_path,
            ],
        )?;
        self.conn.execute(
            "DELETE FROM file_facts WHERE snapshot_id = ?1",
            params![snapshot.id],
        )?;
        self.conn.execute(
            "DELETE FROM repo_edges WHERE snapshot_id = ?1",
            params![snapshot.id],
        )?;
        for fact in facts {
            self.conn.execute(
                "INSERT INTO file_facts (
                    snapshot_id, path, language, bytes, loc, sloc, complexity_total,
                    max_fn_complexity, symbol_count, import_count, fan_in, fan_out,
                    churn_30d, churn_90d, authors_90d, last_changed_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
                params![
                    fact.snapshot_id,
                    fact.path,
                    fact.language,
                    fact.bytes as i64,
                    fact.loc as i64,
                    fact.sloc as i64,
                    fact.complexity_total as i64,
                    fact.max_fn_complexity as i64,
                    fact.symbol_count as i64,
                    fact.import_count as i64,
                    fact.fan_in as i64,
                    fact.fan_out as i64,
                    fact.churn_30d as i64,
                    fact.churn_90d as i64,
                    fact.authors_90d as i64,
                    fact.last_changed_ms.map(|v| v as i64),
                ],
            )?;
        }
        for edge in edges {
            self.conn.execute(
                "INSERT INTO repo_edges (snapshot_id, from_id, to_id, kind, weight)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(snapshot_id, from_id, to_id, kind)
                 DO UPDATE SET weight = weight + excluded.weight",
                params![
                    snapshot.id,
                    edge.from_path,
                    edge.to_path,
                    edge.kind,
                    edge.weight as i64,
                ],
            )?;
        }
        Ok(())
    }

    pub fn file_facts_for_snapshot(&self, snapshot_id: &str) -> Result<Vec<FileFact>> {
        let mut stmt = self.conn.prepare(
            "SELECT snapshot_id, path, language, bytes, loc, sloc, complexity_total,
                    max_fn_complexity, symbol_count, import_count, fan_in, fan_out,
                    churn_30d, churn_90d, authors_90d, last_changed_ms
             FROM file_facts WHERE snapshot_id = ?1 ORDER BY path ASC",
        )?;
        let rows = stmt.query_map(params![snapshot_id], |row| {
            Ok(FileFact {
                snapshot_id: row.get(0)?,
                path: row.get(1)?,
                language: row.get(2)?,
                bytes: row.get::<_, i64>(3)? as u64,
                loc: row.get::<_, i64>(4)? as u32,
                sloc: row.get::<_, i64>(5)? as u32,
                complexity_total: row.get::<_, i64>(6)? as u32,
                max_fn_complexity: row.get::<_, i64>(7)? as u32,
                symbol_count: row.get::<_, i64>(8)? as u32,
                import_count: row.get::<_, i64>(9)? as u32,
                fan_in: row.get::<_, i64>(10)? as u32,
                fan_out: row.get::<_, i64>(11)? as u32,
                churn_30d: row.get::<_, i64>(12)? as u32,
                churn_90d: row.get::<_, i64>(13)? as u32,
                authors_90d: row.get::<_, i64>(14)? as u32,
                last_changed_ms: row.get::<_, Option<i64>>(15)?.map(|v| v as u64),
            })
        })?;
        Ok(rows.filter_map(|row| row.ok()).collect())
    }

    pub fn repo_edges_for_snapshot(&self, snapshot_id: &str) -> Result<Vec<RepoEdge>> {
        let mut stmt = self.conn.prepare(
            "SELECT from_id, to_id, kind, weight
             FROM repo_edges WHERE snapshot_id = ?1
             ORDER BY kind, from_id, to_id",
        )?;
        let rows = stmt.query_map(params![snapshot_id], |row| {
            Ok(RepoEdge {
                from_path: row.get(0)?,
                to_path: row.get(1)?,
                kind: row.get(2)?,
                weight: row.get::<_, i64>(3)? as u32,
            })
        })?;
        Ok(rows.filter_map(|row| row.ok()).collect())
    }

    pub fn hottest_files_for_snapshot(&self, snapshot_id: &str) -> Result<Vec<RankedFile>> {
        self.ranked_files_for_snapshot(snapshot_id, "churn_30d * complexity_total")
    }

    pub fn most_changed_files_for_snapshot(&self, snapshot_id: &str) -> Result<Vec<RankedFile>> {
        self.ranked_files_for_snapshot(snapshot_id, "churn_30d")
    }

    pub fn most_complex_files_for_snapshot(&self, snapshot_id: &str) -> Result<Vec<RankedFile>> {
        self.ranked_files_for_snapshot(snapshot_id, "complexity_total")
    }

    pub fn highest_risk_files_for_snapshot(&self, snapshot_id: &str) -> Result<Vec<RankedFile>> {
        self.ranked_files_for_snapshot(snapshot_id, "churn_30d * authors_90d * complexity_total")
    }

    fn ranked_files_for_snapshot(
        &self,
        snapshot_id: &str,
        value_sql: &str,
    ) -> Result<Vec<RankedFile>> {
        let sql = format!(
            "SELECT path, {value_sql}, complexity_total, churn_30d
             FROM file_facts WHERE snapshot_id = ?1
             ORDER BY {value_sql} DESC, path ASC LIMIT 10"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params![snapshot_id], ranked_file_row)?;
        rows.map(|r| r.map_err(anyhow::Error::from)).collect()
    }

    pub fn pain_hotspots_for_snapshot(
        &self,
        snapshot_id: &str,
        workspace: &str,
        start_ms: u64,
        end_ms: u64,
    ) -> Result<Vec<RankedFile>> {
        let mut stmt = self.conn.prepare(PAIN_HOTSPOTS_SQL)?;
        let rows = stmt.query_map(
            params![snapshot_id, workspace, start_ms as i64, end_ms as i64],
            ranked_file_row,
        )?;
        rows.map(|r| r.map_err(anyhow::Error::from)).collect()
    }

    pub fn tool_rank_rows_in_window(
        &self,
        workspace: &str,
        start_ms: u64,
        end_ms: u64,
    ) -> Result<Vec<RankedTool>> {
        let mut stmt = self.conn.prepare(TOOL_RANK_ROWS_SQL)?;
        let rows = stmt.query_map(
            params![workspace, start_ms as i64, end_ms as i64],
            ranked_tool_row,
        )?;
        rows.map(|r| r.map_err(anyhow::Error::from)).collect()
    }

    pub fn tool_spans_in_window(
        &self,
        workspace: &str,
        start_ms: u64,
        end_ms: u64,
    ) -> Result<Vec<ToolSpanView>> {
        let mut stmt = self.conn.prepare(
            "SELECT span_id, tool, status, lead_time_ms, tokens_in, tokens_out,
                    reasoning_tokens, cost_usd_e6, paths_json,
                    parent_span_id, depth, subtree_cost_usd_e6, subtree_token_count
             FROM (
                 SELECT ts.span_id, ts.tool, ts.status, ts.lead_time_ms,
                        ts.tokens_in, ts.tokens_out, ts.reasoning_tokens,
                        ts.cost_usd_e6, ts.paths_json, ts.parent_span_id,
                        ts.depth, ts.subtree_cost_usd_e6, ts.subtree_token_count,
                        ts.started_at_ms AS sort_ms
                 FROM tool_spans ts
                 JOIN sessions s ON s.id = ts.session_id
                 WHERE s.workspace = ?1
                   AND ts.started_at_ms >= ?2
                   AND ts.started_at_ms <= ?3
                 UNION ALL
                 SELECT ts.span_id, ts.tool, ts.status, ts.lead_time_ms,
                        ts.tokens_in, ts.tokens_out, ts.reasoning_tokens,
                        ts.cost_usd_e6, ts.paths_json, ts.parent_span_id,
                        ts.depth, ts.subtree_cost_usd_e6, ts.subtree_token_count,
                        ts.ended_at_ms AS sort_ms
                 FROM tool_spans ts
                 JOIN sessions s ON s.id = ts.session_id
                 WHERE s.workspace = ?1
                   AND ts.started_at_ms IS NULL
                   AND ts.ended_at_ms >= ?2
                   AND ts.ended_at_ms <= ?3
             )
             ORDER BY sort_ms DESC",
        )?;
        let rows = stmt.query_map(params![workspace, start_ms as i64, end_ms as i64], |row| {
            let paths_json: String = row.get(8)?;
            Ok(ToolSpanView {
                span_id: row.get::<_, Option<String>>(0)?.unwrap_or_default(),
                tool: row
                    .get::<_, Option<String>>(1)?
                    .unwrap_or_else(|| "unknown".into()),
                status: row.get(2)?,
                lead_time_ms: row.get::<_, Option<i64>>(3)?.map(|v| v as u64),
                tokens_in: row.get::<_, Option<i64>>(4)?.map(|v| v as u32),
                tokens_out: row.get::<_, Option<i64>>(5)?.map(|v| v as u32),
                reasoning_tokens: row.get::<_, Option<i64>>(6)?.map(|v| v as u32),
                cost_usd_e6: row.get(7)?,
                paths: serde_json::from_str(&paths_json).unwrap_or_default(),
                parent_span_id: row.get(9)?,
                depth: row.get::<_, Option<i64>>(10)?.unwrap_or(0) as u32,
                subtree_cost_usd_e6: row.get(11)?,
                subtree_token_count: row.get::<_, Option<i64>>(12)?.map(|v| v as u32),
            })
        })?;
        Ok(rows.filter_map(|row| row.ok()).collect())
    }

    pub fn session_span_tree(
        &self,
        session_id: &str,
    ) -> Result<Vec<crate::store::span_tree::SpanNode>> {
        let last_event_seq = self.last_event_seq_for_session(session_id)?;
        if let Some(entry) = self.span_tree_cache.borrow().as_ref()
            && entry.session_id == session_id
            && entry.last_event_seq == last_event_seq
        {
            return Ok(entry.nodes.clone());
        }
        let mut stmt = self.conn.prepare(
            "SELECT span_id, tool, status, lead_time_ms, tokens_in, tokens_out,
                    reasoning_tokens, cost_usd_e6, paths_json,
                    parent_span_id, depth, subtree_cost_usd_e6, subtree_token_count
             FROM tool_spans
             WHERE session_id = ?1
             ORDER BY depth ASC, started_at_ms ASC",
        )?;
        let rows = stmt.query_map(params![session_id], |row| {
            let paths_json: String = row.get(8)?;
            Ok(crate::metrics::types::ToolSpanView {
                span_id: row.get::<_, Option<String>>(0)?.unwrap_or_default(),
                tool: row
                    .get::<_, Option<String>>(1)?
                    .unwrap_or_else(|| "unknown".into()),
                status: row.get(2)?,
                lead_time_ms: row.get::<_, Option<i64>>(3)?.map(|v| v as u64),
                tokens_in: row.get::<_, Option<i64>>(4)?.map(|v| v as u32),
                tokens_out: row.get::<_, Option<i64>>(5)?.map(|v| v as u32),
                reasoning_tokens: row.get::<_, Option<i64>>(6)?.map(|v| v as u32),
                cost_usd_e6: row.get(7)?,
                paths: serde_json::from_str(&paths_json).unwrap_or_default(),
                parent_span_id: row.get(9)?,
                depth: row.get::<_, Option<i64>>(10)?.unwrap_or(0) as u32,
                subtree_cost_usd_e6: row.get(11)?,
                subtree_token_count: row.get::<_, Option<i64>>(12)?.map(|v| v as u32),
            })
        })?;
        let spans: Vec<_> = rows.filter_map(|r| r.ok()).collect();
        let nodes = crate::store::span_tree::build_tree(spans);
        *self.span_tree_cache.borrow_mut() = Some(SpanTreeCacheEntry {
            session_id: session_id.to_string(),
            last_event_seq,
            nodes: nodes.clone(),
        });
        Ok(nodes)
    }

    pub fn last_event_seq_for_session(&self, session_id: &str) -> Result<Option<u64>> {
        let seq = self
            .conn
            .query_row(
                "SELECT MAX(seq) FROM events WHERE session_id = ?1",
                params![session_id],
                |r| r.get::<_, Option<i64>>(0),
            )?
            .map(|v| v as u64);
        Ok(seq)
    }

    /// Sync-shaped tool spans whose session falls in `[start_ms, end_ms]`. Mirrors
    /// `retro_events_in_window` for the spans table so `kaizen telemetry push` can ship
    /// `IngestExportBatch::ToolSpans` next to the events batch. Window matches on
    /// `started_at_ms` first, falling back to `ended_at_ms` for spans that never started a
    /// timer (status-only rows). Workspace filter joins through `sessions.workspace`.
    pub fn tool_spans_sync_rows_in_window(
        &self,
        workspace: &str,
        start_ms: u64,
        end_ms: u64,
    ) -> Result<Vec<ToolSpanSyncRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT span_id, session_id, tool, tool_call_id, status, started_at_ms, ended_at_ms,
                    lead_time_ms, tokens_in, tokens_out, reasoning_tokens, cost_usd_e6, paths_json
             FROM (
                 SELECT ts.span_id, ts.session_id, ts.tool, ts.tool_call_id, ts.status,
                        ts.started_at_ms, ts.ended_at_ms, ts.lead_time_ms, ts.tokens_in,
                        ts.tokens_out, ts.reasoning_tokens, ts.cost_usd_e6, ts.paths_json,
                        ts.started_at_ms AS sort_ms
                 FROM tool_spans ts
                 JOIN sessions s ON s.id = ts.session_id
                 WHERE s.workspace = ?1
                   AND ts.started_at_ms IS NOT NULL
                   AND ts.started_at_ms >= ?2
                   AND ts.started_at_ms <= ?3
                 UNION ALL
                 SELECT ts.span_id, ts.session_id, ts.tool, ts.tool_call_id, ts.status,
                        ts.started_at_ms, ts.ended_at_ms, ts.lead_time_ms, ts.tokens_in,
                        ts.tokens_out, ts.reasoning_tokens, ts.cost_usd_e6, ts.paths_json,
                        ts.ended_at_ms AS sort_ms
                 FROM tool_spans ts
                 JOIN sessions s ON s.id = ts.session_id
                 WHERE s.workspace = ?1
                   AND ts.started_at_ms IS NULL
                   AND ts.ended_at_ms IS NOT NULL
                   AND ts.ended_at_ms >= ?2
                   AND ts.ended_at_ms <= ?3
             )
             ORDER BY sort_ms ASC, span_id ASC",
        )?;
        let rows = stmt.query_map(params![workspace, start_ms as i64, end_ms as i64], |row| {
            let paths_json: String = row.get(12)?;
            Ok(ToolSpanSyncRow {
                span_id: row.get(0)?,
                session_id: row.get(1)?,
                tool: row.get(2)?,
                tool_call_id: row.get(3)?,
                status: row.get(4)?,
                started_at_ms: row.get::<_, Option<i64>>(5)?.map(|v| v as u64),
                ended_at_ms: row.get::<_, Option<i64>>(6)?.map(|v| v as u64),
                lead_time_ms: row.get::<_, Option<i64>>(7)?.map(|v| v as u64),
                tokens_in: row.get::<_, Option<i64>>(8)?.map(|v| v as u32),
                tokens_out: row.get::<_, Option<i64>>(9)?.map(|v| v as u32),
                reasoning_tokens: row.get::<_, Option<i64>>(10)?.map(|v| v as u32),
                cost_usd_e6: row.get(11)?,
                paths: serde_json::from_str(&paths_json).unwrap_or_default(),
            })
        })?;
        Ok(rows.filter_map(|row| row.ok()).collect())
    }

    pub fn tool_spans_for_session(&self, session_id: &str) -> Result<Vec<ToolSpanSyncRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT span_id, session_id, tool, tool_call_id, status, started_at_ms, ended_at_ms, lead_time_ms,
                    tokens_in, tokens_out, reasoning_tokens, cost_usd_e6, paths_json
             FROM tool_spans WHERE session_id = ?1 ORDER BY started_at_ms ASC, span_id ASC",
        )?;
        let rows = stmt.query_map(params![session_id], |row| {
            let paths_json: String = row.get(12)?;
            Ok(ToolSpanSyncRow {
                span_id: row.get(0)?,
                session_id: row.get(1)?,
                tool: row.get(2)?,
                tool_call_id: row.get(3)?,
                status: row.get(4)?,
                started_at_ms: row.get::<_, Option<i64>>(5)?.map(|v| v as u64),
                ended_at_ms: row.get::<_, Option<i64>>(6)?.map(|v| v as u64),
                lead_time_ms: row.get::<_, Option<i64>>(7)?.map(|v| v as u64),
                tokens_in: row.get::<_, Option<i64>>(8)?.map(|v| v as u32),
                tokens_out: row.get::<_, Option<i64>>(9)?.map(|v| v as u32),
                reasoning_tokens: row.get::<_, Option<i64>>(10)?.map(|v| v as u32),
                cost_usd_e6: row.get(11)?,
                paths: serde_json::from_str(&paths_json).unwrap_or_default(),
            })
        })?;
        Ok(rows.filter_map(|row| row.ok()).collect())
    }

    pub fn upsert_eval(&self, eval: &crate::eval::types::EvalRow) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO session_evals
             (id, session_id, judge_model, rubric_id, score, rationale, flagged, created_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                eval.id,
                eval.session_id,
                eval.judge_model,
                eval.rubric_id,
                eval.score,
                eval.rationale,
                eval.flagged as i64,
                eval.created_at_ms as i64,
            ],
        )?;
        Ok(())
    }

    pub fn list_evals_in_window(
        &self,
        start_ms: u64,
        end_ms: u64,
    ) -> rusqlite::Result<Vec<crate::eval::types::EvalRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, session_id, judge_model, rubric_id, score,
                    rationale, flagged, created_at_ms
             FROM session_evals
             WHERE created_at_ms >= ?1 AND created_at_ms < ?2
             ORDER BY created_at_ms ASC",
        )?;
        let rows = stmt.query_map(rusqlite::params![start_ms as i64, end_ms as i64], |r| {
            Ok(crate::eval::types::EvalRow {
                id: r.get(0)?,
                session_id: r.get(1)?,
                judge_model: r.get(2)?,
                rubric_id: r.get(3)?,
                score: r.get(4)?,
                rationale: r.get(5)?,
                flagged: r.get::<_, i64>(6)? != 0,
                created_at_ms: r.get::<_, i64>(7)? as u64,
            })
        })?;
        rows.collect()
    }

    pub fn list_evals_for_session(
        &self,
        session_id: &str,
    ) -> rusqlite::Result<Vec<crate::eval::types::EvalRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, session_id, judge_model, rubric_id, score,
                    rationale, flagged, created_at_ms
             FROM session_evals
             WHERE session_id = ?1
             ORDER BY created_at_ms DESC",
        )?;
        let rows = stmt.query_map(rusqlite::params![session_id], |r| {
            Ok(crate::eval::types::EvalRow {
                id: r.get(0)?,
                session_id: r.get(1)?,
                judge_model: r.get(2)?,
                rubric_id: r.get(3)?,
                score: r.get(4)?,
                rationale: r.get(5)?,
                flagged: r.get::<_, i64>(6)? != 0,
                created_at_ms: r.get::<_, i64>(7)? as u64,
            })
        })?;
        rows.collect()
    }

    pub fn upsert_feedback(&self, r: &crate::feedback::types::FeedbackRecord) -> Result<()> {
        use crate::feedback::types::FeedbackLabel;
        self.conn.execute(
            "INSERT OR REPLACE INTO session_feedback
             (id, session_id, score, label, note, created_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                r.id,
                r.session_id,
                r.score.as_ref().map(|s| s.0 as i64),
                r.label.as_ref().map(FeedbackLabel::to_db_str),
                r.note,
                r.created_at_ms as i64,
            ],
        )?;
        let payload = serde_json::to_string(r).unwrap_or_default();
        self.conn.execute(
            "INSERT INTO sync_outbox (session_id, kind, payload, sent)
             VALUES (?1, 'session_feedback', ?2, 0)",
            rusqlite::params![r.session_id, payload],
        )?;
        Ok(())
    }

    pub fn list_feedback_in_window(
        &self,
        start_ms: u64,
        end_ms: u64,
    ) -> Result<Vec<crate::feedback::types::FeedbackRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, session_id, score, label, note, created_at_ms
             FROM session_feedback
             WHERE created_at_ms >= ?1 AND created_at_ms < ?2
             ORDER BY created_at_ms ASC",
        )?;
        let rows = stmt.query_map(
            rusqlite::params![start_ms as i64, end_ms as i64],
            feedback_row,
        )?;
        rows.map(|r| r.map_err(anyhow::Error::from)).collect()
    }

    pub fn feedback_for_sessions(
        &self,
        ids: &[String],
    ) -> Result<std::collections::HashMap<String, crate::feedback::types::FeedbackRecord>> {
        if ids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }
        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT id, session_id, score, label, note, created_at_ms
             FROM session_feedback WHERE session_id IN ({placeholders})
             ORDER BY created_at_ms DESC"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let params: Vec<&dyn rusqlite::ToSql> =
            ids.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
        let rows = stmt.query_map(params.as_slice(), feedback_row)?;
        let mut map = std::collections::HashMap::new();
        for row in rows {
            let r = row?;
            map.entry(r.session_id.clone()).or_insert(r);
        }
        Ok(map)
    }

    pub fn upsert_session_outcome(&self, row: &SessionOutcomeRow) -> Result<()> {
        self.conn.execute(
            "INSERT INTO session_outcomes (
                session_id, test_passed, test_failed, test_skipped, build_ok, lint_errors,
                revert_lines_14d, pr_open, ci_ok, measured_at_ms, measure_error
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            ON CONFLICT(session_id) DO UPDATE SET
                test_passed=excluded.test_passed,
                test_failed=excluded.test_failed,
                test_skipped=excluded.test_skipped,
                build_ok=excluded.build_ok,
                lint_errors=excluded.lint_errors,
                revert_lines_14d=excluded.revert_lines_14d,
                pr_open=excluded.pr_open,
                ci_ok=excluded.ci_ok,
                measured_at_ms=excluded.measured_at_ms,
                measure_error=excluded.measure_error",
            params![
                row.session_id,
                row.test_passed,
                row.test_failed,
                row.test_skipped,
                row.build_ok.map(bool_to_i64),
                row.lint_errors,
                row.revert_lines_14d,
                row.pr_open,
                row.ci_ok.map(bool_to_i64),
                row.measured_at_ms as i64,
                row.measure_error.as_deref(),
            ],
        )?;
        Ok(())
    }

    pub fn get_session_outcome(&self, session_id: &str) -> Result<Option<SessionOutcomeRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT session_id, test_passed, test_failed, test_skipped, build_ok, lint_errors,
                    revert_lines_14d, pr_open, ci_ok, measured_at_ms, measure_error
             FROM session_outcomes WHERE session_id = ?1",
        )?;
        let row = stmt
            .query_row(params![session_id], outcome_row)
            .optional()?;
        Ok(row)
    }

    /// Outcomes for sessions in `workspace` whose `started_at` falls in the window.
    pub fn list_session_outcomes_in_window(
        &self,
        workspace: &str,
        start_ms: u64,
        end_ms: u64,
    ) -> Result<Vec<SessionOutcomeRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT o.session_id, o.test_passed, o.test_failed, o.test_skipped, o.build_ok, o.lint_errors,
                    o.revert_lines_14d, o.pr_open, o.ci_ok, o.measured_at_ms, o.measure_error
             FROM session_outcomes o
             JOIN sessions s ON s.id = o.session_id
             WHERE s.workspace = ?1 AND s.started_at_ms >= ?2 AND s.started_at_ms <= ?3
             ORDER BY o.measured_at_ms ASC",
        )?;
        let rows = stmt.query_map(
            params![workspace, start_ms as i64, end_ms as i64],
            outcome_row,
        )?;
        rows.map(|r| r.map_err(anyhow::Error::from)).collect()
    }

    pub fn append_session_sample(
        &self,
        session_id: &str,
        ts_ms: u64,
        pid: u32,
        cpu_percent: Option<f64>,
        rss_bytes: Option<u64>,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO session_samples (session_id, ts_ms, pid, cpu_percent, rss_bytes)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                session_id,
                ts_ms as i64,
                pid as i64,
                cpu_percent,
                rss_bytes.map(|b| b as i64)
            ],
        )?;
        Ok(())
    }

    /// Per-session maxima for retro heuristics.
    pub fn list_session_sample_aggs_in_window(
        &self,
        workspace: &str,
        start_ms: u64,
        end_ms: u64,
    ) -> Result<Vec<SessionSampleAgg>> {
        let mut stmt = self.conn.prepare(
            "SELECT ss.session_id, COUNT(*) AS n,
                    MAX(ss.cpu_percent), MAX(ss.rss_bytes)
             FROM session_samples ss
             JOIN sessions s ON s.id = ss.session_id
             WHERE s.workspace = ?1 AND s.started_at_ms >= ?2 AND s.started_at_ms <= ?3
             GROUP BY ss.session_id",
        )?;
        let rows = stmt.query_map(params![workspace, start_ms as i64, end_ms as i64], |r| {
            let sid: String = r.get(0)?;
            let n: i64 = r.get(1)?;
            let max_cpu: Option<f64> = r.get(2)?;
            let max_rss: Option<i64> = r.get(3)?;
            Ok(SessionSampleAgg {
                session_id: sid,
                sample_count: n as u64,
                max_cpu_percent: max_cpu.unwrap_or(0.0),
                max_rss_bytes: max_rss.map(|x| x as u64).unwrap_or(0),
            })
        })?;
        rows.map(|r| r.map_err(anyhow::Error::from)).collect()
    }

    pub fn list_sessions_for_eval(
        &self,
        since_ms: u64,
        min_cost_usd: f64,
    ) -> Result<Vec<crate::core::event::SessionRecord>> {
        let min_cost_e6 = (min_cost_usd * 1_000_000.0) as i64;
        let mut stmt = self.conn.prepare(
            "SELECT s.id, s.agent, s.model, s.workspace, s.started_at_ms, s.ended_at_ms,
                    s.status, s.trace_path, s.start_commit, s.end_commit, s.branch,
                    s.dirty_start, s.dirty_end, s.repo_binding_source, s.prompt_fingerprint,
                    s.parent_session_id, s.agent_version, s.os, s.arch, s.repo_file_count, s.repo_total_loc
             FROM sessions s
             WHERE s.started_at_ms >= ?1
               AND COALESCE((SELECT SUM(e.cost_usd_e6) FROM events e WHERE e.session_id = s.id), 0) >= ?2
               AND NOT EXISTS (SELECT 1 FROM session_evals ev WHERE ev.session_id = s.id)
             ORDER BY s.started_at_ms DESC",
        )?;
        let rows = stmt.query_map(params![since_ms as i64, min_cost_e6], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, i64>(4)?,
                r.get::<_, Option<i64>>(5)?,
                r.get::<_, String>(6)?,
                r.get::<_, String>(7)?,
                r.get::<_, Option<String>>(8)?,
                r.get::<_, Option<String>>(9)?,
                r.get::<_, Option<String>>(10)?,
                r.get::<_, Option<i64>>(11)?,
                r.get::<_, Option<i64>>(12)?,
                r.get::<_, Option<String>>(13)?,
                r.get::<_, Option<String>>(14)?,
                r.get::<_, Option<String>>(15)?,
                r.get::<_, Option<String>>(16)?,
                r.get::<_, Option<String>>(17)?,
                r.get::<_, Option<String>>(18)?,
                r.get::<_, Option<i64>>(19)?,
                r.get::<_, Option<i64>>(20)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (
                id,
                agent,
                model,
                workspace,
                started,
                ended,
                status_str,
                trace,
                start_commit,
                end_commit,
                branch,
                dirty_start,
                dirty_end,
                source,
                prompt_fingerprint,
                parent_session_id,
                agent_version,
                os,
                arch,
                repo_file_count,
                repo_total_loc,
            ) = row?;
            out.push(crate::core::event::SessionRecord {
                id,
                agent,
                model,
                workspace,
                started_at_ms: started as u64,
                ended_at_ms: ended.map(|v| v as u64),
                status: status_from_str(&status_str),
                trace_path: trace,
                start_commit,
                end_commit,
                branch,
                dirty_start: dirty_start.map(i64_to_bool),
                dirty_end: dirty_end.map(i64_to_bool),
                repo_binding_source: source.and_then(|s| if s.is_empty() { None } else { Some(s) }),
                prompt_fingerprint,
                parent_session_id,
                agent_version,
                os,
                arch,
                repo_file_count: repo_file_count.map(|v| v as u32),
                repo_total_loc: repo_total_loc.map(|v| v as u64),
            });
        }
        Ok(out)
    }

    pub fn upsert_prompt_snapshot(&self, snap: &crate::prompt::PromptSnapshot) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO prompt_snapshots
             (fingerprint, captured_at_ms, files_json, total_bytes)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                snap.fingerprint,
                snap.captured_at_ms as i64,
                snap.files_json,
                snap.total_bytes as i64
            ],
        )?;
        Ok(())
    }

    pub fn get_prompt_snapshot(
        &self,
        fingerprint: &str,
    ) -> Result<Option<crate::prompt::PromptSnapshot>> {
        self.conn
            .query_row(
                "SELECT fingerprint, captured_at_ms, files_json, total_bytes
                 FROM prompt_snapshots WHERE fingerprint = ?1",
                params![fingerprint],
                |r| {
                    Ok(crate::prompt::PromptSnapshot {
                        fingerprint: r.get(0)?,
                        captured_at_ms: r.get::<_, i64>(1)? as u64,
                        files_json: r.get(2)?,
                        total_bytes: r.get::<_, i64>(3)? as u64,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn list_prompt_snapshots(&self) -> Result<Vec<crate::prompt::PromptSnapshot>> {
        let mut stmt = self.conn.prepare(
            "SELECT fingerprint, captured_at_ms, files_json, total_bytes
             FROM prompt_snapshots ORDER BY captured_at_ms DESC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(crate::prompt::PromptSnapshot {
                fingerprint: r.get(0)?,
                captured_at_ms: r.get::<_, i64>(1)? as u64,
                files_json: r.get(2)?,
                total_bytes: r.get::<_, i64>(3)? as u64,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Sessions with a non-null prompt_fingerprint in the given window.
    pub fn sessions_with_prompt_fingerprint(
        &self,
        workspace: &str,
        start_ms: u64,
        end_ms: u64,
    ) -> Result<Vec<(String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, prompt_fingerprint FROM sessions
             WHERE workspace = ?1
               AND started_at_ms >= ?2 AND started_at_ms < ?3
               AND prompt_fingerprint IS NOT NULL",
        )?;
        let rows = stmt.query_map(params![workspace, start_ms as i64, end_ms as i64], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }
}

impl Drop for Store {
    fn drop(&mut self) {
        if let Some(writer) = self.search_writer.get_mut().as_mut() {
            let _ = writer.commit();
        }
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn old_session_ids(tx: &rusqlite::Transaction<'_>, cutoff_ms: i64) -> Result<Vec<String>> {
    let mut stmt = tx.prepare("SELECT id FROM sessions WHERE started_at_ms < ?1")?;
    let rows = stmt.query_map(params![cutoff_ms], |r| r.get::<_, String>(0))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

fn mmap_size_bytes_from_mb(raw: Option<&str>) -> i64 {
    raw.and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(DEFAULT_MMAP_MB)
        .saturating_mul(1024)
        .saturating_mul(1024)
        .min(i64::MAX as u64) as i64
}

fn apply_pragmas(conn: &Connection, mode: StoreOpenMode) -> Result<()> {
    let mmap_size = mmap_size_bytes_from_mb(std::env::var("KAIZEN_MMAP_MB").ok().as_deref());
    conn.execute_batch(&format!(
        "
        PRAGMA journal_mode=WAL;
        PRAGMA busy_timeout=5000;
        PRAGMA synchronous=NORMAL;
        PRAGMA cache_size=-65536;
        PRAGMA mmap_size={mmap_size};
        PRAGMA temp_store=MEMORY;
        PRAGMA wal_autocheckpoint=1000;
        "
    ))?;
    if mode == StoreOpenMode::ReadOnlyQuery {
        conn.execute_batch("PRAGMA query_only=ON;")?;
    }
    Ok(())
}

fn count_q(conn: &Connection, sql: &str, workspace: &str) -> Result<u64> {
    Ok(conn.query_row(sql, params![workspace], |r| r.get::<_, i64>(0))? as u64)
}

fn session_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SessionRecord> {
    let status_str: String = row.get(6)?;
    Ok(SessionRecord {
        id: row.get(0)?,
        agent: row.get(1)?,
        model: row.get(2)?,
        workspace: row.get(3)?,
        started_at_ms: row.get::<_, i64>(4)? as u64,
        ended_at_ms: row.get::<_, Option<i64>>(5)?.map(|v| v as u64),
        status: status_from_str(&status_str),
        trace_path: row.get(7)?,
        start_commit: row.get(8)?,
        end_commit: row.get(9)?,
        branch: row.get(10)?,
        dirty_start: row.get::<_, Option<i64>>(11)?.map(i64_to_bool),
        dirty_end: row.get::<_, Option<i64>>(12)?.map(i64_to_bool),
        repo_binding_source: empty_to_none(row.get::<_, String>(13)?),
        prompt_fingerprint: row.get(14)?,
        parent_session_id: row.get(15)?,
        agent_version: row.get(16)?,
        os: row.get(17)?,
        arch: row.get(18)?,
        repo_file_count: row.get::<_, Option<i64>>(19)?.map(|v| v as u32),
        repo_total_loc: row.get::<_, Option<i64>>(20)?.map(|v| v as u64),
    })
}

fn ranked_file_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RankedFile> {
    Ok(RankedFile {
        path: row.get(0)?,
        value: row.get::<_, i64>(1)? as u64,
        complexity_total: row.get::<_, i64>(2)? as u32,
        churn_30d: row.get::<_, i64>(3)? as u32,
    })
}

fn ranked_tool_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RankedTool> {
    Ok(RankedTool {
        tool: row.get(0)?,
        calls: row.get::<_, i64>(1)? as u64,
        p50_ms: row.get::<_, Option<i64>>(2)?.map(|v| v as u64),
        p95_ms: row.get::<_, Option<i64>>(3)?.map(|v| v as u64),
        total_tokens: row.get::<_, i64>(4)? as u64,
        total_reasoning_tokens: row.get::<_, i64>(5)? as u64,
    })
}

fn event_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Event> {
    let payload_str: String = row.get(12)?;
    Ok(Event {
        session_id: row.get(0)?,
        seq: row.get::<_, i64>(1)? as u64,
        ts_ms: row.get::<_, i64>(2)? as u64,
        ts_exact: row.get::<_, i64>(3)? != 0,
        kind: kind_from_str(&row.get::<_, String>(4)?),
        source: source_from_str(&row.get::<_, String>(5)?),
        tool: row.get(6)?,
        tool_call_id: row.get(7)?,
        tokens_in: row.get::<_, Option<i64>>(8)?.map(|v| v as u32),
        tokens_out: row.get::<_, Option<i64>>(9)?.map(|v| v as u32),
        reasoning_tokens: row.get::<_, Option<i64>>(10)?.map(|v| v as u32),
        cost_usd_e6: row.get(11)?,
        payload: serde_json::from_str(&payload_str).unwrap_or(serde_json::Value::Null),
        stop_reason: row.get(13)?,
        latency_ms: row.get::<_, Option<i64>>(14)?.map(|v| v as u32),
        ttft_ms: row.get::<_, Option<i64>>(15)?.map(|v| v as u32),
        retry_count: row.get::<_, Option<i64>>(16)?.map(|v| v as u16),
        context_used_tokens: row.get::<_, Option<i64>>(17)?.map(|v| v as u32),
        context_max_tokens: row.get::<_, Option<i64>>(18)?.map(|v| v as u32),
        cache_creation_tokens: row.get::<_, Option<i64>>(19)?.map(|v| v as u32),
        cache_read_tokens: row.get::<_, Option<i64>>(20)?.map(|v| v as u32),
        system_prompt_tokens: row.get::<_, Option<i64>>(21)?.map(|v| v as u32),
    })
}

fn search_tool_event_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<(String, Event)> {
    Ok((row.get(22)?, event_row(row)?))
}

fn session_filter_sql(workspace: &str, filter: &SessionFilter) -> (String, Vec<Value>) {
    let mut clauses = vec!["workspace = ?".to_string()];
    let mut args = vec![Value::Text(workspace.to_string())];
    if let Some(prefix) = filter.agent_prefix.as_deref().filter(|s| !s.is_empty()) {
        clauses.push("lower(agent) LIKE ? ESCAPE '\\'".to_string());
        args.push(Value::Text(format!("{}%", escape_like(prefix))));
    }
    if let Some(status) = &filter.status {
        clauses.push("status = ?".to_string());
        args.push(Value::Text(format!("{status:?}")));
    }
    if let Some(since_ms) = filter.since_ms {
        clauses.push("started_at_ms >= ?".to_string());
        args.push(Value::Integer(since_ms as i64));
    }
    (format!("WHERE {}", clauses.join(" AND ")), args)
}

fn escape_like(raw: &str) -> String {
    raw.to_lowercase()
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

fn cost_stats(conn: &Connection, workspace: &str) -> Result<(i64, u64)> {
    let cost: i64 = conn.query_row(
        "SELECT COALESCE(SUM(e.cost_usd_e6),0) FROM events e JOIN sessions s ON s.id=e.session_id WHERE s.workspace=?1",
        params![workspace], |r| r.get(0),
    )?;
    let with_cost: i64 = conn.query_row(
        "SELECT COUNT(DISTINCT s.id) FROM sessions s JOIN events e ON e.session_id=s.id WHERE s.workspace=?1 AND e.cost_usd_e6 IS NOT NULL",
        params![workspace], |r| r.get(0),
    )?;
    Ok((cost, with_cost as u64))
}

fn outcome_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<SessionOutcomeRow> {
    let build_raw: Option<i64> = r.get(4)?;
    let ci_raw: Option<i64> = r.get(8)?;
    Ok(SessionOutcomeRow {
        session_id: r.get(0)?,
        test_passed: r.get(1)?,
        test_failed: r.get(2)?,
        test_skipped: r.get(3)?,
        build_ok: build_raw.map(|v| v != 0),
        lint_errors: r.get(5)?,
        revert_lines_14d: r.get(6)?,
        pr_open: r.get(7)?,
        ci_ok: ci_raw.map(|v| v != 0),
        measured_at_ms: r.get::<_, i64>(9)? as u64,
        measure_error: r.get(10)?,
    })
}

fn feedback_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<crate::feedback::types::FeedbackRecord> {
    use crate::feedback::types::{FeedbackLabel, FeedbackRecord, FeedbackScore};
    let score = r
        .get::<_, Option<i64>>(2)?
        .and_then(|v| FeedbackScore::new(v as u8));
    let label = r
        .get::<_, Option<String>>(3)?
        .and_then(|s| FeedbackLabel::from_str_opt(&s));
    Ok(FeedbackRecord {
        id: r.get(0)?,
        session_id: r.get(1)?,
        score,
        label,
        note: r.get(4)?,
        created_at_ms: r.get::<_, i64>(5)? as u64,
    })
}

fn day_label(day_idx: u64) -> &'static str {
    ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"][((day_idx + 4) % 7) as usize]
}

fn sessions_by_day_7(conn: &Connection, workspace: &str, now: u64) -> Result<Vec<(String, u64)>> {
    let week_ago = now.saturating_sub(7 * 86_400_000);
    let mut stmt = conn
        .prepare("SELECT started_at_ms FROM sessions WHERE workspace=?1 AND started_at_ms>=?2")?;
    let days: Vec<u64> = stmt
        .query_map(params![workspace, week_ago as i64], |r| r.get::<_, i64>(0))?
        .filter_map(|r| r.ok())
        .map(|v| v as u64 / 86_400_000)
        .collect();
    let today = now / 86_400_000;
    Ok((0u64..7)
        .map(|i| {
            let d = today.saturating_sub(6 - i);
            (
                day_label(d).to_string(),
                days.iter().filter(|&&x| x == d).count() as u64,
            )
        })
        .collect())
}

fn recent_sessions_3(conn: &Connection, workspace: &str) -> Result<Vec<(SessionRecord, u64)>> {
    let sql = "SELECT s.id,s.agent,s.model,s.workspace,s.started_at_ms,s.ended_at_ms,\
               s.status,s.trace_path,s.start_commit,s.end_commit,s.branch,s.dirty_start,\
               s.dirty_end,s.repo_binding_source,s.prompt_fingerprint,s.parent_session_id,\
               s.agent_version,s.os,s.arch,s.repo_file_count,s.repo_total_loc,\
               COUNT(e.id) FROM sessions s \
               LEFT JOIN events e ON e.session_id=s.id WHERE s.workspace=?1 \
               GROUP BY s.id ORDER BY s.started_at_ms DESC LIMIT 3";
    let mut stmt = conn.prepare(sql)?;
    let out: Vec<(SessionRecord, u64)> = stmt
        .query_map(params![workspace], |r| {
            let st: String = r.get(6)?;
            Ok((
                SessionRecord {
                    id: r.get(0)?,
                    agent: r.get(1)?,
                    model: r.get(2)?,
                    workspace: r.get(3)?,
                    started_at_ms: r.get::<_, i64>(4)? as u64,
                    ended_at_ms: r.get::<_, Option<i64>>(5)?.map(|v| v as u64),
                    status: status_from_str(&st),
                    trace_path: r.get(7)?,
                    start_commit: r.get(8)?,
                    end_commit: r.get(9)?,
                    branch: r.get(10)?,
                    dirty_start: r.get::<_, Option<i64>>(11)?.map(i64_to_bool),
                    dirty_end: r.get::<_, Option<i64>>(12)?.map(i64_to_bool),
                    repo_binding_source: empty_to_none(r.get::<_, String>(13)?),
                    prompt_fingerprint: r.get(14)?,
                    parent_session_id: r.get(15)?,
                    agent_version: r.get(16)?,
                    os: r.get(17)?,
                    arch: r.get(18)?,
                    repo_file_count: r.get::<_, Option<i64>>(19)?.map(|v| v as u32),
                    repo_total_loc: r.get::<_, Option<i64>>(20)?.map(|v| v as u64),
                },
                r.get::<_, i64>(21)? as u64,
            ))
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(out)
}

fn top_tools_5(conn: &Connection, workspace: &str) -> Result<Vec<(String, u64)>> {
    let mut stmt = conn.prepare(
        "SELECT tool, COUNT(*) FROM events e JOIN sessions s ON s.id=e.session_id \
         WHERE s.workspace=?1 AND tool IS NOT NULL GROUP BY tool ORDER BY COUNT(*) DESC LIMIT 5",
    )?;
    let out: Vec<(String, u64)> = stmt
        .query_map(params![workspace], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? as u64))
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(out)
}

fn status_from_str(s: &str) -> SessionStatus {
    match s {
        "Running" => SessionStatus::Running,
        "Waiting" => SessionStatus::Waiting,
        "Idle" => SessionStatus::Idle,
        _ => SessionStatus::Done,
    }
}

fn projector_legacy_mode() -> bool {
    std::env::var("KAIZEN_PROJECTOR").is_ok_and(|v| v == "legacy")
}

fn is_stop_event(e: &Event) -> bool {
    if !matches!(e.kind, EventKind::Hook) {
        return false;
    }
    e.payload
        .get("event")
        .and_then(|v| v.as_str())
        .or_else(|| e.payload.get("hook_event_name").and_then(|v| v.as_str()))
        == Some("Stop")
}

fn kind_from_str(s: &str) -> EventKind {
    match s {
        "ToolCall" => EventKind::ToolCall,
        "ToolResult" => EventKind::ToolResult,
        "Message" => EventKind::Message,
        "Error" => EventKind::Error,
        "Cost" => EventKind::Cost,
        "Hook" => EventKind::Hook,
        "Lifecycle" => EventKind::Lifecycle,
        _ => EventKind::Hook,
    }
}

fn source_from_str(s: &str) -> EventSource {
    match s {
        "Tail" => EventSource::Tail,
        "Hook" => EventSource::Hook,
        _ => EventSource::Proxy,
    }
}

fn ensure_schema_columns(conn: &Connection) -> Result<()> {
    ensure_column(conn, "sessions", "start_commit", "TEXT")?;
    ensure_column(conn, "sessions", "end_commit", "TEXT")?;
    ensure_column(conn, "sessions", "branch", "TEXT")?;
    ensure_column(conn, "sessions", "dirty_start", "INTEGER")?;
    ensure_column(conn, "sessions", "dirty_end", "INTEGER")?;
    ensure_column(
        conn,
        "sessions",
        "repo_binding_source",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    ensure_column(conn, "events", "ts_exact", "INTEGER NOT NULL DEFAULT 0")?;
    ensure_column(conn, "events", "tool_call_id", "TEXT")?;
    ensure_column(conn, "events", "reasoning_tokens", "INTEGER")?;
    ensure_column(conn, "events", "stop_reason", "TEXT")?;
    ensure_column(conn, "events", "latency_ms", "INTEGER")?;
    ensure_column(conn, "events", "ttft_ms", "INTEGER")?;
    ensure_column(conn, "events", "retry_count", "INTEGER")?;
    ensure_column(conn, "events", "context_used_tokens", "INTEGER")?;
    ensure_column(conn, "events", "context_max_tokens", "INTEGER")?;
    ensure_column(conn, "events", "cache_creation_tokens", "INTEGER")?;
    ensure_column(conn, "events", "cache_read_tokens", "INTEGER")?;
    ensure_column(conn, "events", "system_prompt_tokens", "INTEGER")?;
    ensure_column(
        conn,
        "sync_outbox",
        "kind",
        "TEXT NOT NULL DEFAULT 'events'",
    )?;
    ensure_column(
        conn,
        "experiments",
        "state",
        "TEXT NOT NULL DEFAULT 'Draft'",
    )?;
    ensure_column(conn, "experiments", "concluded_at_ms", "INTEGER")?;
    ensure_column(conn, "sessions", "prompt_fingerprint", "TEXT")?;
    ensure_column(conn, "sessions", "parent_session_id", "TEXT")?;
    ensure_column(conn, "sessions", "agent_version", "TEXT")?;
    ensure_column(conn, "sessions", "os", "TEXT")?;
    ensure_column(conn, "sessions", "arch", "TEXT")?;
    ensure_column(conn, "sessions", "repo_file_count", "INTEGER")?;
    ensure_column(conn, "sessions", "repo_total_loc", "INTEGER")?;
    ensure_column(conn, "tool_spans", "parent_span_id", "TEXT")?;
    ensure_column(conn, "tool_spans", "depth", "INTEGER NOT NULL DEFAULT 0")?;
    ensure_column(conn, "tool_spans", "subtree_cost_usd_e6", "INTEGER")?;
    ensure_column(conn, "tool_spans", "subtree_token_count", "INTEGER")?;
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS tool_spans_parent ON tool_spans(parent_span_id);
         CREATE INDEX IF NOT EXISTS tool_spans_session_depth ON tool_spans(session_id, depth);",
    )?;
    Ok(())
}

fn ensure_column(conn: &Connection, table: &str, column: &str, sql_type: &str) -> Result<()> {
    if has_column(conn, table, column)? {
        return Ok(());
    }
    conn.execute(
        &format!("ALTER TABLE {table} ADD COLUMN {column} {sql_type}"),
        [],
    )?;
    Ok(())
}

fn has_column(conn: &Connection, table: &str, column: &str) -> Result<bool> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
    Ok(rows.filter_map(|r| r.ok()).any(|name| name == column))
}

fn bool_to_i64(v: bool) -> i64 {
    if v { 1 } else { 0 }
}

fn i64_to_bool(v: i64) -> bool {
    v != 0
}

fn empty_to_none(s: String) -> Option<String> {
    if s.is_empty() { None } else { Some(s) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashSet;
    use tempfile::TempDir;

    fn make_session(id: &str) -> SessionRecord {
        SessionRecord {
            id: id.to_string(),
            agent: "cursor".to_string(),
            model: None,
            workspace: "/ws".to_string(),
            started_at_ms: 1000,
            ended_at_ms: None,
            status: SessionStatus::Done,
            trace_path: "/trace".to_string(),
            start_commit: None,
            end_commit: None,
            branch: None,
            dirty_start: None,
            dirty_end: None,
            repo_binding_source: None,
            prompt_fingerprint: None,
            parent_session_id: None,
            agent_version: None,
            os: None,
            arch: None,
            repo_file_count: None,
            repo_total_loc: None,
        }
    }

    fn make_event(session_id: &str, seq: u64) -> Event {
        Event {
            session_id: session_id.to_string(),
            seq,
            ts_ms: 1000 + seq * 100,
            ts_exact: false,
            kind: EventKind::ToolCall,
            source: EventSource::Tail,
            tool: Some("read_file".to_string()),
            tool_call_id: Some(format!("call_{seq}")),
            tokens_in: None,
            tokens_out: None,
            reasoning_tokens: None,
            cost_usd_e6: None,
            stop_reason: None,
            latency_ms: None,
            ttft_ms: None,
            retry_count: None,
            context_used_tokens: None,
            context_max_tokens: None,
            cache_creation_tokens: None,
            cache_read_tokens: None,
            system_prompt_tokens: None,
            payload: json!({}),
        }
    }

    #[test]
    fn open_and_wal_mode() {
        let dir = TempDir::new().unwrap();
        let store = Store::open(&dir.path().join("kaizen.db")).unwrap();
        let mode: String = store
            .conn
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .unwrap();
        assert_eq!(mode, "wal");
    }

    #[test]
    fn open_applies_phase0_pragmas() {
        let dir = TempDir::new().unwrap();
        let store = Store::open(&dir.path().join("kaizen.db")).unwrap();
        let synchronous: i64 = store
            .conn
            .query_row("PRAGMA synchronous", [], |r| r.get(0))
            .unwrap();
        let cache_size: i64 = store
            .conn
            .query_row("PRAGMA cache_size", [], |r| r.get(0))
            .unwrap();
        let temp_store: i64 = store
            .conn
            .query_row("PRAGMA temp_store", [], |r| r.get(0))
            .unwrap();
        let wal_autocheckpoint: i64 = store
            .conn
            .query_row("PRAGMA wal_autocheckpoint", [], |r| r.get(0))
            .unwrap();
        assert_eq!(synchronous, 1);
        assert_eq!(cache_size, -65_536);
        assert_eq!(temp_store, 2);
        assert_eq!(wal_autocheckpoint, 1_000);
        assert_eq!(mmap_size_bytes_from_mb(Some("64")), 67_108_864);
    }

    #[test]
    fn read_only_open_sets_query_only() {
        let dir = TempDir::new().unwrap();
        let db = dir.path().join("kaizen.db");
        Store::open(&db).unwrap();
        let store = Store::open_read_only(&db).unwrap();
        let query_only: i64 = store
            .conn
            .query_row("PRAGMA query_only", [], |r| r.get(0))
            .unwrap();
        assert_eq!(query_only, 1);
    }

    #[test]
    fn phase0_indexes_exist() {
        let dir = TempDir::new().unwrap();
        let store = Store::open(&dir.path().join("kaizen.db")).unwrap();
        for name in [
            "tool_spans_session_idx",
            "tool_spans_started_idx",
            "session_samples_ts_idx",
            "events_ts_idx",
            "feedback_session_idx",
        ] {
            let found: i64 = store
                .conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name=?1",
                    params![name],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(found, 1, "{name}");
        }
    }

    #[test]
    fn upsert_and_get_session() {
        let dir = TempDir::new().unwrap();
        let store = Store::open(&dir.path().join("kaizen.db")).unwrap();
        let s = make_session("s1");
        store.upsert_session(&s).unwrap();

        let got = store.get_session("s1").unwrap().unwrap();
        assert_eq!(got.id, "s1");
        assert_eq!(got.status, SessionStatus::Done);
    }

    #[test]
    fn append_and_list_events_round_trip() {
        let dir = TempDir::new().unwrap();
        let store = Store::open(&dir.path().join("kaizen.db")).unwrap();
        let s = make_session("s2");
        store.upsert_session(&s).unwrap();
        store.append_event(&make_event("s2", 0)).unwrap();
        store.append_event(&make_event("s2", 1)).unwrap();

        let sessions = store.list_sessions("/ws").unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id, "s2");
    }

    #[test]
    fn list_sessions_page_orders_and_counts() {
        let dir = TempDir::new().unwrap();
        let store = Store::open(&dir.path().join("kaizen.db")).unwrap();
        let mut a = make_session("a");
        a.started_at_ms = 2_000;
        let mut b = make_session("b");
        b.started_at_ms = 2_000;
        let mut c = make_session("c");
        c.started_at_ms = 1_000;
        store.upsert_session(&c).unwrap();
        store.upsert_session(&b).unwrap();
        store.upsert_session(&a).unwrap();

        let page = store
            .list_sessions_page("/ws", 0, 2, SessionFilter::default())
            .unwrap();
        assert_eq!(page.total, 3);
        assert_eq!(page.next_offset, Some(2));
        assert_eq!(
            page.rows.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
            vec!["a", "b"]
        );

        let all = store.list_sessions("/ws").unwrap();
        assert_eq!(
            all.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
            vec!["a", "b", "c"]
        );
    }

    #[test]
    fn list_sessions_page_filters_in_sql_shape() {
        let dir = TempDir::new().unwrap();
        let store = Store::open(&dir.path().join("kaizen.db")).unwrap();
        let mut cursor = make_session("cursor");
        cursor.agent = "Cursor".into();
        cursor.started_at_ms = 2_000;
        cursor.status = SessionStatus::Running;
        let mut claude = make_session("claude");
        claude.agent = "claude".into();
        claude.started_at_ms = 3_000;
        store.upsert_session(&cursor).unwrap();
        store.upsert_session(&claude).unwrap();

        let page = store
            .list_sessions_page(
                "/ws",
                0,
                10,
                SessionFilter {
                    agent_prefix: Some("cur".into()),
                    status: Some(SessionStatus::Running),
                    since_ms: Some(1_500),
                },
            )
            .unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.rows[0].id, "cursor");
    }

    #[test]
    fn incremental_session_helpers_find_new_rows_and_statuses() {
        let dir = TempDir::new().unwrap();
        let store = Store::open(&dir.path().join("kaizen.db")).unwrap();
        let mut old = make_session("old");
        old.started_at_ms = 1_000;
        let mut new = make_session("new");
        new.started_at_ms = 2_000;
        new.status = SessionStatus::Running;
        store.upsert_session(&old).unwrap();
        store.upsert_session(&new).unwrap();

        let rows = store.list_sessions_started_after("/ws", 1_500).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "new");

        store
            .update_session_status("new", SessionStatus::Done)
            .unwrap();
        let statuses = store.session_statuses(&["new".to_string()]).unwrap();
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].status, SessionStatus::Done);
    }

    #[test]
    fn summary_stats_empty() {
        let dir = TempDir::new().unwrap();
        let store = Store::open(&dir.path().join("kaizen.db")).unwrap();
        let stats = store.summary_stats("/ws").unwrap();
        assert_eq!(stats.session_count, 0);
        assert_eq!(stats.total_cost_usd_e6, 0);
    }

    #[test]
    fn summary_stats_counts_sessions() {
        let dir = TempDir::new().unwrap();
        let store = Store::open(&dir.path().join("kaizen.db")).unwrap();
        store.upsert_session(&make_session("a")).unwrap();
        store.upsert_session(&make_session("b")).unwrap();
        let stats = store.summary_stats("/ws").unwrap();
        assert_eq!(stats.session_count, 2);
        assert_eq!(stats.by_agent.len(), 1);
        assert_eq!(stats.by_agent[0].0, "cursor");
        assert_eq!(stats.by_agent[0].1, 2);
    }

    #[test]
    fn list_events_for_session_round_trip() {
        let dir = TempDir::new().unwrap();
        let store = Store::open(&dir.path().join("kaizen.db")).unwrap();
        store.upsert_session(&make_session("s4")).unwrap();
        store.append_event(&make_event("s4", 0)).unwrap();
        store.append_event(&make_event("s4", 1)).unwrap();
        let events = store.list_events_for_session("s4").unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].seq, 0);
        assert_eq!(events[1].seq, 1);
    }

    #[test]
    fn list_events_page_uses_inclusive_seq_cursor() {
        let dir = TempDir::new().unwrap();
        let store = Store::open(&dir.path().join("kaizen.db")).unwrap();
        store.upsert_session(&make_session("paged")).unwrap();
        for seq in 0..5 {
            store.append_event(&make_event("paged", seq)).unwrap();
        }
        let first = store.list_events_page("paged", 0, 2).unwrap();
        assert_eq!(first.iter().map(|e| e.seq).collect::<Vec<_>>(), vec![0, 1]);
        let second = store
            .list_events_page("paged", first[1].seq + 1, 2)
            .unwrap();
        assert_eq!(second.iter().map(|e| e.seq).collect::<Vec<_>>(), vec![2, 3]);
    }

    #[test]
    fn append_event_dedup() {
        let dir = TempDir::new().unwrap();
        let store = Store::open(&dir.path().join("kaizen.db")).unwrap();
        store.upsert_session(&make_session("s5")).unwrap();
        store.append_event(&make_event("s5", 0)).unwrap();
        // Duplicate — should be silently ignored
        store.append_event(&make_event("s5", 0)).unwrap();
        let events = store.list_events_for_session("s5").unwrap();
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn span_tree_cache_hits_empty_and_invalidates_on_append() {
        let dir = TempDir::new().unwrap();
        let store = Store::open(&dir.path().join("kaizen.db")).unwrap();
        assert!(store.session_span_tree("missing").unwrap().is_empty());
        assert!(store.span_tree_cache.borrow().is_some());

        store.upsert_session(&make_session("tree")).unwrap();
        let call = make_event("tree", 0);
        store.append_event(&call).unwrap();
        assert!(store.span_tree_cache.borrow().is_none());
        assert!(store.session_span_tree("tree").unwrap().is_empty());
        assert!(store.span_tree_cache.borrow().is_some());
        let mut result = make_event("tree", 1);
        result.kind = EventKind::ToolResult;
        result.tool_call_id = call.tool_call_id.clone();
        store.append_event(&result).unwrap();
        assert!(store.span_tree_cache.borrow().is_none());
        let first = store.session_span_tree("tree").unwrap();
        assert_eq!(first.len(), 1);
        assert!(store.span_tree_cache.borrow().is_some());
        store.append_event(&make_event("tree", 2)).unwrap();
        assert!(store.span_tree_cache.borrow().is_none());
    }

    #[test]
    fn tool_spans_in_window_uses_started_then_ended_fallback() {
        let dir = TempDir::new().unwrap();
        let store = Store::open(&dir.path().join("kaizen.db")).unwrap();
        store.upsert_session(&make_session("spans")).unwrap();
        for (id, started, ended) in [
            ("started", Some(200_i64), None),
            ("fallback", None, Some(250_i64)),
            ("outside", Some(400_i64), None),
            ("too_old", None, Some(50_i64)),
            ("started_wins", Some(500_i64), Some(200_i64)),
        ] {
            store
                .conn
                .execute(
                    "INSERT INTO tool_spans
                     (span_id, session_id, tool, status, started_at_ms, ended_at_ms, paths_json)
                     VALUES (?1, 'spans', 'read', 'done', ?2, ?3, '[]')",
                    params![id, started, ended],
                )
                .unwrap();
        }
        let rows = store.tool_spans_in_window("/ws", 100, 300).unwrap();
        let ids = rows.into_iter().map(|r| r.span_id).collect::<Vec<_>>();
        assert_eq!(ids, vec!["fallback".to_string(), "started".to_string()]);
    }

    #[test]
    fn tool_spans_sync_rows_in_window_returns_session_id_with_filtering() {
        let dir = TempDir::new().unwrap();
        let store = Store::open(&dir.path().join("kaizen.db")).unwrap();
        store.upsert_session(&make_session("s1")).unwrap();
        for (id, started, ended) in [
            ("inside_started", Some(150_i64), None),
            ("inside_ended_only", None, Some(220_i64)),
            ("after_window", Some(400_i64), None),
            ("before_window", None, Some(50_i64)),
        ] {
            store
                .conn
                .execute(
                    "INSERT INTO tool_spans
                     (span_id, session_id, tool, status, started_at_ms, ended_at_ms, paths_json)
                     VALUES (?1, 's1', 'read', 'done', ?2, ?3, '[]')",
                    params![id, started, ended],
                )
                .unwrap();
        }
        let rows = store
            .tool_spans_sync_rows_in_window("/ws", 100, 300)
            .unwrap();
        let ids: Vec<_> = rows.iter().map(|r| r.span_id.as_str()).collect();
        assert_eq!(ids, vec!["inside_started", "inside_ended_only"]);
        assert!(rows.iter().all(|r| r.session_id == "s1"));
    }

    #[test]
    fn upsert_idempotent() {
        let dir = TempDir::new().unwrap();
        let store = Store::open(&dir.path().join("kaizen.db")).unwrap();
        let mut s = make_session("s3");
        store.upsert_session(&s).unwrap();
        s.status = SessionStatus::Running;
        store.upsert_session(&s).unwrap();

        let got = store.get_session("s3").unwrap().unwrap();
        assert_eq!(got.status, SessionStatus::Running);
    }

    #[test]
    fn append_event_indexes_path_from_payload() {
        let dir = TempDir::new().unwrap();
        let store = Store::open(&dir.path().join("kaizen.db")).unwrap();
        store.upsert_session(&make_session("sx")).unwrap();
        let mut ev = make_event("sx", 0);
        ev.payload = json!({"input": {"path": "src/lib.rs"}});
        store.append_event(&ev).unwrap();
        let ft = store.files_touched_in_window("/ws", 0, 10_000).unwrap();
        assert_eq!(ft, vec![("sx".to_string(), "src/lib.rs".to_string())]);
    }

    #[test]
    fn update_session_status_changes_status() {
        let dir = TempDir::new().unwrap();
        let store = Store::open(&dir.path().join("kaizen.db")).unwrap();
        store.upsert_session(&make_session("s6")).unwrap();
        store
            .update_session_status("s6", SessionStatus::Running)
            .unwrap();
        let got = store.get_session("s6").unwrap().unwrap();
        assert_eq!(got.status, SessionStatus::Running);
    }

    #[test]
    fn prune_sessions_removes_old_rows_and_keeps_recent() {
        let dir = TempDir::new().unwrap();
        let store = Store::open(&dir.path().join("kaizen.db")).unwrap();
        let mut old = make_session("old");
        old.started_at_ms = 1_000;
        let mut new = make_session("new");
        new.started_at_ms = 9_000_000_000_000;
        store.upsert_session(&old).unwrap();
        store.upsert_session(&new).unwrap();
        store.append_event(&make_event("old", 0)).unwrap();

        let stats = store.prune_sessions_started_before(5_000).unwrap();
        assert_eq!(
            stats,
            PruneStats {
                sessions_removed: 1,
                events_removed: 1,
            }
        );
        assert!(store.get_session("old").unwrap().is_none());
        assert!(store.get_session("new").unwrap().is_some());
        let sessions = store.list_sessions("/ws").unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id, "new");
    }

    #[test]
    fn append_event_indexes_rules_from_payload() {
        let dir = TempDir::new().unwrap();
        let store = Store::open(&dir.path().join("kaizen.db")).unwrap();
        store.upsert_session(&make_session("sr")).unwrap();
        let mut ev = make_event("sr", 0);
        ev.payload = json!({"path": ".cursor/rules/my-rule.mdc"});
        store.append_event(&ev).unwrap();
        let r = store.rules_used_in_window("/ws", 0, 10_000).unwrap();
        assert_eq!(r, vec![("sr".to_string(), "my-rule".to_string())]);
    }

    #[test]
    fn guidance_report_counts_skill_and_rule_sessions() {
        let dir = TempDir::new().unwrap();
        let store = Store::open(&dir.path().join("kaizen.db")).unwrap();
        store.upsert_session(&make_session("sx")).unwrap();
        let mut ev = make_event("sx", 0);
        ev.payload =
            json!({"text": "read .cursor/skills/tdd/SKILL.md and .cursor/rules/style.mdc"});
        ev.cost_usd_e6 = Some(500_000);
        store.append_event(&ev).unwrap();

        let mut skill_slugs = HashSet::new();
        skill_slugs.insert("tdd".into());
        let mut rule_slugs = HashSet::new();
        rule_slugs.insert("style".into());

        let rep = store
            .guidance_report("/ws", 0, 10_000, &skill_slugs, &rule_slugs)
            .unwrap();
        assert_eq!(rep.sessions_in_window, 1);
        let tdd = rep
            .rows
            .iter()
            .find(|r| r.id == "tdd" && r.kind == GuidanceKind::Skill)
            .unwrap();
        assert_eq!(tdd.sessions, 1);
        assert!(tdd.on_disk);
        let style = rep
            .rows
            .iter()
            .find(|r| r.id == "style" && r.kind == GuidanceKind::Rule)
            .unwrap();
        assert_eq!(style.sessions, 1);
        assert!(style.on_disk);
    }

    #[test]
    fn prune_sessions_removes_rules_used_rows() {
        let dir = TempDir::new().unwrap();
        let store = Store::open(&dir.path().join("kaizen.db")).unwrap();
        let mut old = make_session("old_r");
        old.started_at_ms = 1_000;
        store.upsert_session(&old).unwrap();
        let mut ev = make_event("old_r", 0);
        ev.payload = json!({"path": ".cursor/rules/x.mdc"});
        store.append_event(&ev).unwrap();

        store.prune_sessions_started_before(5_000).unwrap();
        let n: i64 = store
            .conn
            .query_row("SELECT COUNT(*) FROM rules_used", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0);
    }
}
