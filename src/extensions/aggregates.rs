// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::store::Store;
use anyhow::Result;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SessionAggregate {
    pub session_id: String,
    pub event_count: u64,
    pub tool_call_count: u64,
    pub error_count: u64,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub reasoning_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cost_usd_e6: i64,
    pub first_event_ms: Option<u64>,
    pub last_event_ms: Option<u64>,
    pub rebuilt_at_ms: u64,
}

pub fn rebuild_workspace(store: &Store, workspace: &str) -> Result<usize> {
    store
        .list_sessions(workspace)?
        .iter()
        .map(|s| upsert_session(store, &s.id).map(|_| 1usize))
        .sum()
}

pub fn upsert_session(store: &Store, session_id: &str) -> Result<SessionAggregate> {
    let row = aggregate_raw(store, session_id, now_ms())?;
    store.conn().execute(
        "INSERT INTO session_aggregates (
            session_id, event_count, tool_call_count, error_count, tokens_in,
            tokens_out, reasoning_tokens, cache_read_tokens, cache_creation_tokens,
            cost_usd_e6, first_event_ms, last_event_ms, rebuilt_at_ms
        ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)
        ON CONFLICT(session_id) DO UPDATE SET
            event_count=excluded.event_count, tool_call_count=excluded.tool_call_count,
            error_count=excluded.error_count, tokens_in=excluded.tokens_in,
            tokens_out=excluded.tokens_out, reasoning_tokens=excluded.reasoning_tokens,
            cache_read_tokens=excluded.cache_read_tokens,
            cache_creation_tokens=excluded.cache_creation_tokens,
            cost_usd_e6=excluded.cost_usd_e6, first_event_ms=excluded.first_event_ms,
            last_event_ms=excluded.last_event_ms, rebuilt_at_ms=excluded.rebuilt_at_ms",
        params![
            row.session_id,
            row.event_count as i64,
            row.tool_call_count as i64,
            row.error_count as i64,
            row.tokens_in as i64,
            row.tokens_out as i64,
            row.reasoning_tokens as i64,
            row.cache_read_tokens as i64,
            row.cache_creation_tokens as i64,
            row.cost_usd_e6,
            row.first_event_ms.map(|v| v as i64),
            row.last_event_ms.map(|v| v as i64),
            row.rebuilt_at_ms as i64,
        ],
    )?;
    Ok(row)
}

pub fn get(store: &Store, session_id: &str) -> Result<Option<SessionAggregate>> {
    store
        .conn()
        .query_row(
            "SELECT session_id, event_count, tool_call_count, error_count, tokens_in,
                    tokens_out, reasoning_tokens, cache_read_tokens, cache_creation_tokens,
                    cost_usd_e6, first_event_ms, last_event_ms, rebuilt_at_ms
             FROM session_aggregates WHERE session_id = ?1",
            [session_id],
            map_aggregate,
        )
        .optional()
        .map_err(Into::into)
}

fn aggregate_raw(store: &Store, session_id: &str, rebuilt_at_ms: u64) -> Result<SessionAggregate> {
    store
        .conn()
        .query_row(
            "SELECT COUNT(*), COALESCE(SUM(kind='ToolCall'),0), COALESCE(SUM(kind='Error'),0),
                COALESCE(SUM(tokens_in),0), COALESCE(SUM(tokens_out),0),
                COALESCE(SUM(reasoning_tokens),0), COALESCE(SUM(cache_read_tokens),0),
                COALESCE(SUM(cache_creation_tokens),0), COALESCE(SUM(cost_usd_e6),0),
                MIN(ts_ms), MAX(ts_ms)
         FROM events WHERE session_id = ?1",
            [session_id],
            |row| {
                Ok(SessionAggregate {
                    session_id: session_id.to_string(),
                    event_count: row.get::<_, i64>(0)? as u64,
                    tool_call_count: row.get::<_, i64>(1)? as u64,
                    error_count: row.get::<_, i64>(2)? as u64,
                    tokens_in: row.get::<_, i64>(3)? as u64,
                    tokens_out: row.get::<_, i64>(4)? as u64,
                    reasoning_tokens: row.get::<_, i64>(5)? as u64,
                    cache_read_tokens: row.get::<_, i64>(6)? as u64,
                    cache_creation_tokens: row.get::<_, i64>(7)? as u64,
                    cost_usd_e6: row.get(8)?,
                    first_event_ms: row.get::<_, Option<i64>>(9)?.map(|v| v as u64),
                    last_event_ms: row.get::<_, Option<i64>>(10)?.map(|v| v as u64),
                    rebuilt_at_ms,
                })
            },
        )
        .map_err(Into::into)
}

fn map_aggregate(row: &rusqlite::Row<'_>) -> rusqlite::Result<SessionAggregate> {
    Ok(SessionAggregate {
        session_id: row.get(0)?,
        event_count: row.get::<_, i64>(1)? as u64,
        tool_call_count: row.get::<_, i64>(2)? as u64,
        error_count: row.get::<_, i64>(3)? as u64,
        tokens_in: row.get::<_, i64>(4)? as u64,
        tokens_out: row.get::<_, i64>(5)? as u64,
        reasoning_tokens: row.get::<_, i64>(6)? as u64,
        cache_read_tokens: row.get::<_, i64>(7)? as u64,
        cache_creation_tokens: row.get::<_, i64>(8)? as u64,
        cost_usd_e6: row.get(9)?,
        first_event_ms: row.get::<_, Option<i64>>(10)?.map(|v| v as u64),
        last_event_ms: row.get::<_, Option<i64>>(11)?.map(|v| v as u64),
        rebuilt_at_ms: row.get::<_, i64>(12)? as u64,
    })
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
