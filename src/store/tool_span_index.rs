//! Rebuild tool spans from one session event stream.

use crate::core::event::{Event, EventKind, EventSource};
use crate::store::event_index::paths_from_event_payload;
use anyhow::Result;
use rusqlite::{Connection, params};
use std::collections::{BTreeSet, HashMap};

#[derive(Debug, Default)]
struct SpanBuilder {
    span_id: String,
    session_id: String,
    tool: Option<String>,
    tool_call_id: Option<String>,
    hook_start_ms: Option<u64>,
    hook_end_ms: Option<u64>,
    call_start_ms: Option<u64>,
    result_end_ms: Option<u64>,
    call_start_exact: bool,
    result_end_exact: bool,
    tokens_in: Option<u32>,
    tokens_out: Option<u32>,
    reasoning_tokens: Option<u32>,
    cost_usd_e6: Option<i64>,
    paths: BTreeSet<String>,
    has_call: bool,
    has_end: bool,
}

pub fn rebuild_tool_spans_for_session(conn: &Connection, session_id: &str) -> Result<()> {
    let events = load_session_events(conn, session_id)?;
    clear_session_spans(conn, session_id)?;
    let spans = build_spans(&events);
    for span in spans {
        let lead = span
            .hook_start_ms
            .zip(span.hook_end_ms)
            .map(|(a, b)| b.saturating_sub(a))
            .or_else(|| {
                if span.call_start_exact && span.result_end_exact {
                    span.call_start_ms
                        .zip(span.result_end_ms)
                        .map(|(a, b)| b.saturating_sub(a))
                } else {
                    None
                }
            });
        let started = span.hook_start_ms.or(span.call_start_ms);
        let ended = span.hook_end_ms.or(span.result_end_ms);
        let status = if started.is_some() && ended.is_some() {
            "done"
        } else {
            "orphaned"
        };
        conn.execute(
            "INSERT INTO tool_spans (
                span_id, session_id, tool, tool_call_id, status,
                started_at_ms, ended_at_ms, lead_time_ms,
                tokens_in, tokens_out, reasoning_tokens, cost_usd_e6, paths_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                span.span_id,
                span.session_id,
                span.tool,
                span.tool_call_id,
                status,
                started.map(|v| v as i64),
                ended.map(|v| v as i64),
                lead.map(|v| v as i64),
                span.tokens_in.map(|v| v as i64),
                span.tokens_out.map(|v| v as i64),
                span.reasoning_tokens.map(|v| v as i64),
                span.cost_usd_e6,
                serde_json::to_string(&span.paths.iter().cloned().collect::<Vec<_>>())?,
            ],
        )?;
        for path in span.paths {
            conn.execute(
                "INSERT INTO tool_span_paths (span_id, path) VALUES (?1, ?2)",
                params![span.span_id, path],
            )?;
        }
    }
    Ok(())
}

fn clear_session_spans(conn: &Connection, session_id: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM tool_span_paths
         WHERE span_id IN (SELECT span_id FROM tool_spans WHERE session_id = ?1)",
        params![session_id],
    )?;
    conn.execute(
        "DELETE FROM tool_spans WHERE session_id = ?1",
        params![session_id],
    )?;
    Ok(())
}

fn build_spans(events: &[Event]) -> Vec<SpanBuilder> {
    let mut spans: HashMap<String, SpanBuilder> = HashMap::new();
    let mut open_order: Vec<String> = Vec::new();
    for event in events {
        if !matches!(
            event.kind,
            EventKind::ToolCall | EventKind::ToolResult | EventKind::Hook
        ) {
            continue;
        }
        match event.kind {
            EventKind::ToolCall => handle_tool_call(event, &mut spans, &mut open_order),
            EventKind::ToolResult => handle_tool_result(event, &mut spans, &open_order),
            EventKind::Hook => handle_hook(event, &mut spans, &mut open_order),
            _ => {}
        }
    }
    spans.into_values().collect()
}

fn handle_tool_call(
    event: &Event,
    spans: &mut HashMap<String, SpanBuilder>,
    open_order: &mut Vec<String>,
) {
    let tool = event.tool.clone();
    let existing = tool
        .as_deref()
        .and_then(|name| find_open_without_call(spans, open_order, name));
    let span_id = event
        .tool_call_id
        .clone()
        .unwrap_or_else(|| existing.unwrap_or_else(|| synthetic_span_id(event)));
    let span = spans.entry(span_id.clone()).or_insert_with(|| SpanBuilder {
        span_id: span_id.clone(),
        session_id: event.session_id.clone(),
        tool: tool.clone(),
        tool_call_id: event.tool_call_id.clone(),
        ..Default::default()
    });
    span.tool = tool;
    span.tool_call_id = event.tool_call_id.clone();
    span.call_start_ms = Some(event.ts_ms);
    span.call_start_exact = event.ts_exact;
    span.tokens_in = pick_u32(span.tokens_in, event.tokens_in);
    span.tokens_out = pick_u32(span.tokens_out, event.tokens_out);
    span.reasoning_tokens = pick_u32(span.reasoning_tokens, event.reasoning_tokens);
    span.cost_usd_e6 = pick_i64(span.cost_usd_e6, event.cost_usd_e6);
    span.paths.extend(paths_from_event_payload(&event.payload));
    span.has_call = true;
    if !open_order.iter().any(|id| id == &span_id) {
        open_order.push(span_id);
    }
}

fn handle_tool_result(
    event: &Event,
    spans: &mut HashMap<String, SpanBuilder>,
    open_order: &[String],
) {
    let Some(span_id) = match_span_id(event, spans, open_order) else {
        return;
    };
    let Some(span) = spans.get_mut(&span_id) else {
        return;
    };
    span.result_end_ms = Some(event.ts_ms);
    span.result_end_exact = event.ts_exact;
    span.tokens_in = pick_u32(span.tokens_in, event.tokens_in);
    span.tokens_out = pick_u32(span.tokens_out, event.tokens_out);
    span.reasoning_tokens = pick_u32(span.reasoning_tokens, event.reasoning_tokens);
    span.cost_usd_e6 = pick_i64(span.cost_usd_e6, event.cost_usd_e6);
    span.paths.extend(paths_from_event_payload(&event.payload));
    span.has_end = true;
}

fn handle_hook(
    event: &Event,
    spans: &mut HashMap<String, SpanBuilder>,
    open_order: &mut Vec<String>,
) {
    let Some(kind) = hook_kind(&event.payload) else {
        return;
    };
    let tool = hook_tool(&event.payload);
    let span_id = event
        .tool_call_id
        .clone()
        .or_else(|| {
            tool.as_deref()
                .and_then(|name| find_open_same_tool(spans, open_order, name))
        })
        .unwrap_or_else(|| synthetic_span_id(event));
    let span = spans.entry(span_id.clone()).or_insert_with(|| SpanBuilder {
        span_id: span_id.clone(),
        session_id: event.session_id.clone(),
        tool: tool.clone(),
        tool_call_id: event.tool_call_id.clone(),
        ..Default::default()
    });
    span.tool = span.tool.clone().or(tool);
    span.tool_call_id = span.tool_call_id.clone().or(event.tool_call_id.clone());
    span.paths.extend(paths_from_event_payload(&event.payload));
    match kind {
        "pre" => span.hook_start_ms = Some(event.ts_ms),
        "post" => {
            span.hook_end_ms = Some(event.ts_ms);
            span.has_end = true;
        }
        _ => {}
    }
    if !open_order.iter().any(|id| id == &span_id) {
        open_order.push(span_id);
    }
}

fn load_session_events(conn: &Connection, session_id: &str) -> Result<Vec<Event>> {
    let mut stmt = conn.prepare(
        "SELECT session_id, seq, ts_ms, COALESCE(ts_exact, 0), kind, source, tool,
                tool_call_id, tokens_in, tokens_out, reasoning_tokens, cost_usd_e6, payload
         FROM events WHERE session_id = ?1 ORDER BY ts_ms ASC, seq ASC",
    )?;
    let rows = stmt.query_map(params![session_id], |row| {
        let kind = match row.get::<_, String>(4)?.as_str() {
            "ToolCall" => EventKind::ToolCall,
            "ToolResult" => EventKind::ToolResult,
            "Message" => EventKind::Message,
            "Error" => EventKind::Error,
            "Cost" => EventKind::Cost,
            _ => EventKind::Hook,
        };
        let source = match row.get::<_, String>(5)?.as_str() {
            "Tail" => EventSource::Tail,
            "Proxy" => EventSource::Proxy,
            _ => EventSource::Hook,
        };
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
            tokens_in: row.get::<_, Option<i64>>(8)?.map(|v| v as u32),
            tokens_out: row.get::<_, Option<i64>>(9)?.map(|v| v as u32),
            reasoning_tokens: row.get::<_, Option<i64>>(10)?.map(|v| v as u32),
            cost_usd_e6: row.get(11)?,
            payload: serde_json::from_str(&payload).unwrap_or(serde_json::Value::Null),
        })
    })?;
    Ok(rows.filter_map(|row| row.ok()).collect())
}

fn match_span_id(
    event: &Event,
    spans: &HashMap<String, SpanBuilder>,
    open_order: &[String],
) -> Option<String> {
    if let Some(id) = event
        .tool_call_id
        .as_ref()
        .filter(|id| spans.contains_key(*id))
    {
        return Some(id.clone());
    }
    event
        .tool
        .as_deref()
        .and_then(|name| find_open_same_tool(spans, open_order, name))
        .or_else(|| open_order.last().cloned())
}

fn find_open_without_call(
    spans: &HashMap<String, SpanBuilder>,
    open_order: &[String],
    tool: &str,
) -> Option<String> {
    open_order.iter().rev().find_map(|id| {
        spans.get(id).and_then(|span| {
            if span.tool.as_deref() == Some(tool) && !span.has_call {
                Some(id.clone())
            } else {
                None
            }
        })
    })
}

fn find_open_same_tool(
    spans: &HashMap<String, SpanBuilder>,
    open_order: &[String],
    tool: &str,
) -> Option<String> {
    open_order.iter().rev().find_map(|id| {
        spans.get(id).and_then(|span| {
            if span.tool.as_deref() == Some(tool) && !span.has_end {
                Some(id.clone())
            } else {
                None
            }
        })
    })
}

fn synthetic_span_id(event: &Event) -> String {
    format!("{}:{}:{}", event.session_id, event.seq, event.ts_ms)
}

fn hook_kind(payload: &serde_json::Value) -> Option<&'static str> {
    let raw = payload
        .get("event")
        .and_then(|v| v.as_str())
        .or_else(|| payload.get("hook_event_name").and_then(|v| v.as_str()))?;
    match raw {
        "PreToolUse" | "pre_tool_use" => Some("pre"),
        "PostToolUse" | "post_tool_use" => Some("post"),
        _ => None,
    }
}

fn hook_tool(payload: &serde_json::Value) -> Option<String> {
    ["tool_name", "tool", "name"]
        .iter()
        .find_map(|k| payload.get(k).and_then(|v| v.as_str()))
        .map(ToOwned::to_owned)
}

fn pick_u32(current: Option<u32>, next: Option<u32>) -> Option<u32> {
    next.or(current)
}

fn pick_i64(current: Option<i64>, next: Option<i64>) -> Option<i64> {
    next.or(current)
}
