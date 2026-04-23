//! Sync SQLite store. WAL mode, schema migrations as ordered SQL strings.

use crate::core::config::try_team_salt;
use crate::core::event::{Event, EventKind, EventSource, SessionRecord, SessionStatus};
use crate::sync::context::SyncIngestContext;
use crate::sync::outbound::outbound_event_from_row;
use crate::sync::redact::redact_payload;
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
    "CREATE UNIQUE INDEX IF NOT EXISTS events_session_seq_idx ON events(session_id, seq)",
    "CREATE TABLE IF NOT EXISTS sync_state (
        k TEXT PRIMARY KEY,
        v TEXT NOT NULL
    )",
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
pub struct SummaryStats {
    pub session_count: u64,
    pub total_cost_usd_e6: i64,
    pub by_agent: Vec<(String, u64)>,
    pub by_model: Vec<(String, u64)>,
    pub top_tools: Vec<(String, u64)>,
}

pub struct Store {
    conn: Connection,
}

impl Store {
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
        Ok(Self { conn })
    }

    pub fn upsert_session(&self, s: &SessionRecord) -> Result<()> {
        self.conn.execute(
            "INSERT INTO sessions (id, agent, model, workspace, started_at_ms, ended_at_ms, status, trace_path)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(id) DO UPDATE SET
               agent=excluded.agent, model=excluded.model, workspace=excluded.workspace,
               started_at_ms=excluded.started_at_ms, ended_at_ms=excluded.ended_at_ms,
               status=excluded.status, trace_path=excluded.trace_path",
            params![
                s.id,
                s.agent,
                s.model,
                s.workspace,
                s.started_at_ms as i64,
                s.ended_at_ms.map(|v| v as i64),
                format!("{:?}", s.status),
                s.trace_path,
            ],
        )?;
        Ok(())
    }

    pub fn append_event(&self, e: &Event) -> Result<()> {
        self.append_event_with_sync(e, None)
    }

    /// Append event; when `ctx` is set and sync is configured, enqueue one redacted outbox row.
    pub fn append_event_with_sync(
        &self,
        e: &Event,
        ctx: Option<&SyncIngestContext>,
    ) -> Result<()> {
        let payload = serde_json::to_string(&e.payload)?;
        self.conn.execute(
            "INSERT OR IGNORE INTO events (session_id, seq, ts_ms, kind, source, tool, tokens_in, tokens_out, cost_usd_e6, payload)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                e.session_id,
                e.seq as i64,
                e.ts_ms as i64,
                format!("{:?}", e.kind),
                format!("{:?}", e.source),
                e.tool,
                e.tokens_in.map(|v| v as i64),
                e.tokens_out.map(|v| v as i64),
                e.cost_usd_e6,
                payload,
            ],
        )?;
        if self.conn.changes() == 0 {
            return Ok(());
        }
        let Some(ctx) = ctx else {
            return Ok(());
        };
        let sync = &ctx.sync;
        if sync.endpoint.is_empty()
            || sync.team_token.is_empty()
            || sync.team_id.is_empty()
        {
            return Ok(());
        }
        let Some(salt) = try_team_salt(sync) else {
            tracing::warn!("sync outbox skipped: set sync.team_salt_hex (64 hex chars) in ~/.kaizen/config.toml");
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
            "INSERT INTO sync_outbox (session_id, payload, sent) VALUES (?1, ?2, 0)",
            params![e.session_id, row],
        )?;
        Ok(())
    }

    pub fn list_outbox_pending(&self, limit: usize) -> Result<Vec<(i64, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, payload FROM sync_outbox WHERE sent = 0 ORDER BY id ASC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn mark_outbox_sent(&self, ids: &[i64]) -> Result<()> {
        for id in ids {
            self.conn.execute(
                "UPDATE sync_outbox SET sent = 1 WHERE id = ?1",
                params![id],
            )?;
        }
        Ok(())
    }

    pub fn outbox_pending_count(&self) -> Result<u64> {
        let c: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM sync_outbox WHERE sent = 0",
            [],
            |r| r.get(0),
        )?;
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
            .query_row(
                "SELECT v FROM sync_state WHERE k = 'last_error'",
                [],
                |r| r.get::<_, String>(0),
            )
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
            "SELECT id, agent, model, workspace, started_at_ms, ended_at_ms, status, trace_path
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
            ))
        })?;

        let mut out = Vec::new();
        for row in rows {
            let (id, agent, model, workspace, started, ended, status_str, trace) = row?;
            out.push(SessionRecord {
                id,
                agent,
                model,
                workspace,
                started_at_ms: started as u64,
                ended_at_ms: ended.map(|v| v as u64),
                status: status_from_str(&status_str),
                trace_path: trace,
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
            "SELECT session_id, seq, ts_ms, kind, source, tool, tokens_in, tokens_out, cost_usd_e6, payload
             FROM events WHERE session_id = ?1 ORDER BY seq ASC",
        )?;
        let rows = stmt.query_map(params![session_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<i64>>(6)?,
                row.get::<_, Option<i64>>(7)?,
                row.get::<_, Option<i64>>(8)?,
                row.get::<_, String>(9)?,
            ))
        })?;

        let mut events = Vec::new();
        for row in rows {
            let (
                sid,
                seq,
                ts_ms,
                kind_str,
                source_str,
                tool,
                tokens_in,
                tokens_out,
                cost_usd_e6,
                payload_str,
            ) = row?;
            events.push(Event {
                session_id: sid,
                seq: seq as u64,
                ts_ms: ts_ms as u64,
                kind: kind_from_str(&kind_str),
                source: source_from_str(&source_str),
                tool,
                tokens_in: tokens_in.map(|v| v as u32),
                tokens_out: tokens_out.map(|v| v as u32),
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

    pub fn get_session(&self, id: &str) -> Result<Option<SessionRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, agent, model, workspace, started_at_ms, ended_at_ms, status, trace_path
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
            ))
        })?;

        if let Some(row) = rows.next() {
            let (id, agent, model, workspace, started, ended, status_str, trace) = row?;
            Ok(Some(SessionRecord {
                id,
                agent,
                model,
                workspace,
                started_at_ms: started as u64,
                ended_at_ms: ended.map(|v| v as u64),
                status: status_from_str(&status_str),
                trace_path: trace,
            }))
        } else {
            Ok(None)
        }
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
               s.status,s.trace_path,COUNT(e.id) FROM sessions s \
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
                },
                r.get::<_, i64>(8)? as u64,
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
        }
    }

    fn make_event(session_id: &str, seq: u64) -> Event {
        Event {
            session_id: session_id.to_string(),
            seq,
            ts_ms: 1000 + seq * 100,
            kind: EventKind::ToolCall,
            source: EventSource::Tail,
            tool: Some("read_file".to_string()),
            tokens_in: None,
            tokens_out: None,
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
