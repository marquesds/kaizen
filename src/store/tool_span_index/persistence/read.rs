// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::core::event::{Event, EventKind, EventSource};
use anyhow::Result;
use rusqlite::{Connection, Row, params};

const SESSION_EVENTS_SQL: &str = "
    SELECT session_id, seq, ts_ms, COALESCE(ts_exact, 0), kind, source, tool,
           tool_call_id, tokens_in, tokens_out, reasoning_tokens, cost_usd_e6, payload
    FROM events WHERE session_id = ?1 ORDER BY ts_ms ASC, seq ASC";

pub(super) fn load_session_events(conn: &Connection, session_id: &str) -> Result<Vec<Event>> {
    let mut statement = conn.prepare(SESSION_EVENTS_SQL)?;
    let rows = statement.query_map(params![session_id], event_from_row)?;
    Ok(rows.filter_map(|row| row.ok()).collect())
}

fn event_from_row(row: &Row<'_>) -> rusqlite::Result<Event> {
    let kind = event_kind(&row.get::<_, String>(4)?);
    let source = event_source(&row.get::<_, String>(5)?);
    let payload: String = row.get(12)?;
    Ok(Event {
        session_id: row.get(0)?,
        seq: row.get::<_, i64>(1)? as u64,
        ts_ms: row.get::<_, i64>(2)? as u64,
        ts_exact: row.get::<_, i64>(3)? != 0,
        kind,
        source,
        tool: row.get(6)?,
        tool_call_id: row.get(7)?,
        tokens_in: row.get::<_, Option<i64>>(8)?.map(|value| value as u32),
        tokens_out: row.get::<_, Option<i64>>(9)?.map(|value| value as u32),
        reasoning_tokens: row.get::<_, Option<i64>>(10)?.map(|value| value as u32),
        cost_usd_e6: row.get(11)?,
        stop_reason: None,
        latency_ms: None,
        ttft_ms: None,
        retry_count: None,
        context_used_tokens: None,
        context_max_tokens: None,
        cache_creation_tokens: None,
        cache_read_tokens: None,
        system_prompt_tokens: None,
        payload: serde_json::from_str(&payload).unwrap_or(serde_json::Value::Null),
    })
}

fn event_kind(raw: &str) -> EventKind {
    match raw {
        "ToolCall" => EventKind::ToolCall,
        "ToolResult" => EventKind::ToolResult,
        "Message" => EventKind::Message,
        "Error" => EventKind::Error,
        "Cost" => EventKind::Cost,
        _ => EventKind::Hook,
    }
}

fn event_source(raw: &str) -> EventSource {
    match raw {
        "Tail" => EventSource::Tail,
        "Proxy" => EventSource::Proxy,
        _ => EventSource::Hook,
    }
}
