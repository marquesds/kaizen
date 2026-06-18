// SPDX-License-Identifier: AGPL-3.0-or-later

use super::super::ToolSpanRecord;
use anyhow::Result;
use rusqlite::{Connection, params};

const CLEAR_PATHS_SQL: &str = "
    DELETE FROM tool_span_paths
    WHERE span_id IN (SELECT span_id FROM tool_spans WHERE session_id = ?1)";
const CLEAR_SPANS_SQL: &str = "DELETE FROM tool_spans WHERE session_id = ?1";
const DELETE_PATHS_SQL: &str = "DELETE FROM tool_span_paths WHERE span_id = ?1";
const INSERT_PATH_SQL: &str = "INSERT INTO tool_span_paths (span_id, path) VALUES (?1, ?2)";
const UPSERT_SPAN_SQL: &str = "
    INSERT INTO tool_spans (
        span_id, session_id, tool, tool_call_id, status,
        started_at_ms, ended_at_ms, lead_time_ms,
        tokens_in, tokens_out, reasoning_tokens, cost_usd_e6, paths_json,
        parent_span_id, depth, subtree_cost_usd_e6, subtree_token_count
    ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)
    ON CONFLICT(span_id) DO UPDATE SET
        session_id=excluded.session_id,
        tool=excluded.tool,
        tool_call_id=excluded.tool_call_id,
        status=excluded.status,
        started_at_ms=excluded.started_at_ms,
        ended_at_ms=excluded.ended_at_ms,
        lead_time_ms=excluded.lead_time_ms,
        tokens_in=excluded.tokens_in,
        tokens_out=excluded.tokens_out,
        reasoning_tokens=excluded.reasoning_tokens,
        cost_usd_e6=excluded.cost_usd_e6,
        paths_json=excluded.paths_json,
        parent_span_id=excluded.parent_span_id,
        depth=excluded.depth,
        subtree_cost_usd_e6=excluded.subtree_cost_usd_e6,
        subtree_token_count=excluded.subtree_token_count";

pub(crate) fn clear_session_spans(conn: &Connection, session_id: &str) -> Result<()> {
    conn.execute(CLEAR_PATHS_SQL, params![session_id])?;
    conn.execute(CLEAR_SPANS_SQL, params![session_id])?;
    Ok(())
}

pub(crate) fn upsert_tool_span_record(conn: &Connection, span: &ToolSpanRecord) -> Result<()> {
    let persisted = namespaced_record(span);
    upsert_span(conn, &persisted)?;
    replace_paths(conn, &persisted)
}

fn namespaced_record(span: &ToolSpanRecord) -> ToolSpanRecord {
    ToolSpanRecord {
        span_id: persisted_span_id(&span.session_id, &span.span_id),
        parent_span_id: span
            .parent_span_id
            .as_deref()
            .map(|id| persisted_span_id(&span.session_id, id)),
        ..span.clone()
    }
}

fn persisted_span_id(session_id: &str, span_id: &str) -> String {
    format!("v1:{}:{session_id}:{span_id}", session_id.len())
}

fn upsert_span(conn: &Connection, span: &ToolSpanRecord) -> Result<()> {
    conn.execute(
        UPSERT_SPAN_SQL,
        params![
            &span.span_id,
            &span.session_id,
            span.tool.as_deref(),
            span.tool_call_id.as_deref(),
            &span.status,
            span.started_at_ms.map(|value| value as i64),
            span.ended_at_ms.map(|value| value as i64),
            span.lead_time_ms.map(|value| value as i64),
            span.tokens_in.map(|value| value as i64),
            span.tokens_out.map(|value| value as i64),
            span.reasoning_tokens.map(|value| value as i64),
            span.cost_usd_e6,
            serde_json::to_string(&span.paths)?,
            span.parent_span_id.as_deref(),
            span.depth as i64,
            span.subtree_cost_usd_e6,
            span.subtree_token_count.map(|value| value as i64),
        ],
    )?;
    Ok(())
}

fn replace_paths(conn: &Connection, span: &ToolSpanRecord) -> Result<()> {
    conn.execute(DELETE_PATHS_SQL, params![&span.span_id])?;
    for path in &span.paths {
        conn.execute(INSERT_PATH_SQL, params![&span.span_id, path])?;
    }
    Ok(())
}
