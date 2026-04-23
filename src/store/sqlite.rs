// SPDX-License-Identifier: AGPL-3.0-or-later
//! Sync SQLite store. WAL mode, schema migrations as ordered SQL strings.

use crate::core::config::try_team_salt;
use crate::core::event::{Event, EventKind, EventSource, SessionRecord, SessionStatus};
use crate::metrics::types::{FileFact, RepoEdge, RepoSnapshotRecord, ToolSpanView};
use crate::store::event_index::index_event_derived;
use crate::store::tool_span_index::rebuild_tool_spans_for_session;
use crate::sync::context::SyncIngestContext;
use crate::sync::outbound::outbound_event_from_row;
use crate::sync::redact::redact_payload;
use crate::sync::smart::enqueue_tool_spans_for_session;
use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;

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
];

/// Per-workspace activity dashboard stats.
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

pub struct Store {
    conn: Connection,
}

impl Store {
    pub(crate) fn conn(&self) -> &Connection {
        &self.conn
    }

    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn =
            Connection::open(path).with_context(|| format!("open db: {}", path.display()))?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")?;
        for sql in MIGRATIONS {
            conn.execute_batch(sql)?;
        }
        ensure_schema_columns(&conn)?;
        Ok(Self { conn })
    }

    pub fn upsert_session(&self, s: &SessionRecord) -> Result<()> {
        self.conn.execute(
            "INSERT INTO sessions (
                id, agent, model, workspace, started_at_ms, ended_at_ms, status, trace_path,
                start_commit, end_commit, branch, dirty_start, dirty_end, repo_binding_source
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
             ON CONFLICT(id) DO UPDATE SET
               agent=excluded.agent, model=excluded.model, workspace=excluded.workspace,
               started_at_ms=excluded.started_at_ms, ended_at_ms=excluded.ended_at_ms,
               status=excluded.status, trace_path=excluded.trace_path,
               start_commit=excluded.start_commit, end_commit=excluded.end_commit,
               branch=excluded.branch, dirty_start=excluded.dirty_start,
               dirty_end=excluded.dirty_end, repo_binding_source=excluded.repo_binding_source",
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
        let payload = serde_json::to_string(&e.payload)?;
        self.conn.execute(
            "INSERT OR IGNORE INTO events (
                session_id, seq, ts_ms, ts_exact, kind, source, tool, tool_call_id,
                tokens_in, tokens_out, reasoning_tokens, cost_usd_e6, payload
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
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
            ],
        )?;
        if self.conn.changes() == 0 {
            return Ok(());
        }
        index_event_derived(&self.conn, e)?;
        rebuild_tool_spans_for_session(&self.conn, &e.session_id)?;
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
        self.conn.execute(
            "INSERT INTO sync_outbox (session_id, kind, payload, sent) VALUES (?1, 'events', ?2, 0)",
            params![e.session_id, row],
        )?;
        enqueue_tool_spans_for_session(self, &e.session_id, ctx)?;
        Ok(())
    }

    pub fn list_outbox_pending(&self, limit: usize) -> Result<Vec<(i64, String, String)>> {
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

    pub fn list_sessions(&self, workspace: &str) -> Result<Vec<SessionRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, agent, model, workspace, started_at_ms, ended_at_ms, status, trace_path,
                    start_commit, end_commit, branch, dirty_start, dirty_end, repo_binding_source
             FROM sessions WHERE workspace = ?1 ORDER BY started_at_ms DESC",
        )?;
        let rows = stmt.query_map(params![workspace], |row| {
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
            ) = row?;
            out.push(SessionRecord {
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
            });
        }
        Ok(out)
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
        let mut stmt = self.conn.prepare(
            "SELECT session_id, seq, ts_ms, COALESCE(ts_exact, 0), kind, source, tool, tool_call_id,
                    tokens_in, tokens_out, reasoning_tokens, cost_usd_e6, payload
             FROM events WHERE session_id = ?1 ORDER BY seq ASC",
        )?;
        let rows = stmt.query_map(params![session_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<String>>(7)?,
                row.get::<_, Option<i64>>(8)?,
                row.get::<_, Option<i64>>(9)?,
                row.get::<_, Option<i64>>(10)?,
                row.get::<_, Option<i64>>(11)?,
                row.get::<_, String>(12)?,
            ))
        })?;

        let mut events = Vec::new();
        for row in rows {
            let (
                sid,
                seq,
                ts_ms,
                ts_exact,
                kind_str,
                source_str,
                tool,
                tool_call_id,
                tokens_in,
                tokens_out,
                reasoning_tokens,
                cost_usd_e6,
                payload_str,
            ) = row?;
            events.push(Event {
                session_id: sid,
                seq: seq as u64,
                ts_ms: ts_ms as u64,
                ts_exact: ts_exact != 0,
                kind: kind_from_str(&kind_str),
                source: source_from_str(&source_str),
                tool,
                tool_call_id,
                tokens_in: tokens_in.map(|v| v as u32),
                tokens_out: tokens_out.map(|v| v as u32),
                reasoning_tokens: reasoning_tokens.map(|v| v as u32),
                cost_usd_e6,
                payload: serde_json::from_str(&payload_str).unwrap_or(serde_json::Value::Null),
            });
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
                    s.start_commit, s.end_commit, s.branch, s.dirty_start, s.dirty_end, s.repo_binding_source
             FROM events e
             JOIN sessions s ON s.id = e.session_id
             WHERE s.workspace = ?1 AND e.ts_ms >= ?2 AND e.ts_ms <= ?3
             ORDER BY e.ts_ms ASC, e.session_id ASC, e.seq ASC",
        )?;
        let rows = stmt.query_map(params![workspace, start_ms as i64, end_ms as i64], |row| {
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
                    payload: serde_json::from_str(&payload_str).unwrap_or(serde_json::Value::Null),
                },
            ))
        })?;

        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
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
                 WHERE e.session_id = ft.session_id
                   AND e.ts_ms >= ?2 AND e.ts_ms <= ?3
               )
             ORDER BY ft.session_id, ft.path",
        )?;
        let out: Vec<(String, String)> = stmt
            .query_map(params![workspace, start_ms as i64, end_ms as i64], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })?
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
                 WHERE e.session_id = su.session_id AND e.ts_ms >= ?2
               )
             ORDER BY su.skill",
        )?;
        let out: Vec<String> = stmt
            .query_map(params![workspace, since_ms as i64], |r| r.get(0))?
            .filter_map(|r| r.ok())
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
                 WHERE e.session_id = su.session_id
                   AND e.ts_ms >= ?2 AND e.ts_ms <= ?3
               )
             ORDER BY su.session_id, su.skill",
        )?;
        let out: Vec<(String, String)> = stmt
            .query_map(params![workspace, start_ms as i64, end_ms as i64], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(out)
    }

    pub fn get_session(&self, id: &str) -> Result<Option<SessionRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, agent, model, workspace, started_at_ms, ended_at_ms, status, trace_path,
                    start_commit, end_commit, branch, dirty_start, dirty_end, repo_binding_source
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
                 VALUES (?1, ?2, ?3, ?4, ?5)",
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

    pub fn tool_spans_in_window(
        &self,
        workspace: &str,
        start_ms: u64,
        end_ms: u64,
    ) -> Result<Vec<ToolSpanView>> {
        let mut stmt = self.conn.prepare(
            "SELECT ts.tool, ts.status, ts.lead_time_ms, ts.tokens_in, ts.tokens_out,
                    ts.reasoning_tokens, ts.cost_usd_e6, ts.paths_json
             FROM tool_spans ts
             JOIN sessions s ON s.id = ts.session_id
             WHERE s.workspace = ?1
               AND COALESCE(ts.started_at_ms, ts.ended_at_ms, 0) >= ?2
               AND COALESCE(ts.started_at_ms, ts.ended_at_ms, 0) <= ?3
             ORDER BY COALESCE(ts.started_at_ms, ts.ended_at_ms, 0) DESC",
        )?;
        let rows = stmt.query_map(params![workspace, start_ms as i64, end_ms as i64], |row| {
            let paths_json: String = row.get(7)?;
            Ok(ToolSpanView {
                tool: row
                    .get::<_, Option<String>>(0)?
                    .unwrap_or_else(|| "unknown".into()),
                status: row.get(1)?,
                lead_time_ms: row.get::<_, Option<i64>>(2)?.map(|v| v as u64),
                tokens_in: row.get::<_, Option<i64>>(3)?.map(|v| v as u32),
                tokens_out: row.get::<_, Option<i64>>(4)?.map(|v| v as u32),
                reasoning_tokens: row.get::<_, Option<i64>>(5)?.map(|v| v as u32),
                cost_usd_e6: row.get(6)?,
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
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn count_q(conn: &Connection, sql: &str, workspace: &str) -> Result<u64> {
    Ok(conn.query_row(sql, params![workspace], |r| r.get::<_, i64>(0))? as u64)
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
               s.dirty_end,s.repo_binding_source,COUNT(e.id) FROM sessions s \
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
                },
                r.get::<_, i64>(14)? as u64,
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

fn kind_from_str(s: &str) -> EventKind {
    match s {
        "ToolCall" => EventKind::ToolCall,
        "ToolResult" => EventKind::ToolResult,
        "Message" => EventKind::Message,
        "Error" => EventKind::Error,
        "Cost" => EventKind::Cost,
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
}
