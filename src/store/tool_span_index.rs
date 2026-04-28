// SPDX-License-Identifier: AGPL-3.0-or-later
//! Rebuild tool spans from one session event stream.

use crate::core::event::{Event, EventKind, EventSource};
use crate::store::event_index::paths_from_event_payload;
use anyhow::Result;
use rusqlite::{Connection, params};
use std::collections::{BTreeSet, HashMap};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct SpanBuilder {
    pub span_id: String,
    pub session_id: String,
    pub tool: Option<String>,
    pub tool_call_id: Option<String>,
    pub hook_start_ms: Option<u64>,
    pub hook_end_ms: Option<u64>,
    pub call_start_ms: Option<u64>,
    pub result_end_ms: Option<u64>,
    pub call_start_exact: bool,
    pub result_end_exact: bool,
    pub tokens_in: Option<u32>,
    pub tokens_out: Option<u32>,
    pub reasoning_tokens: Option<u32>,
    pub cost_usd_e6: Option<i64>,
    pub paths: BTreeSet<String>,
    pub has_call: bool,
    pub has_end: bool,
    pub parent_span_id: Option<String>,
    pub depth: u32,
    pub subtree_cost_usd_e6: Option<i64>,
    pub subtree_token_count: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolSpanRecord {
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
    pub parent_span_id: Option<String>,
    pub depth: u32,
    pub subtree_cost_usd_e6: Option<i64>,
    pub subtree_token_count: Option<u32>,
}

pub fn rebuild_tool_spans_for_session(conn: &Connection, session_id: &str) -> Result<()> {
    let events = load_session_events(conn, session_id)?;
    clear_session_spans(conn, session_id)?;
    for span in final_span_records(&events) {
        upsert_tool_span_record(conn, &span)?;
    }
    Ok(())
}

pub(crate) fn span_start(s: &SpanBuilder) -> Option<u64> {
    s.hook_start_ms.or(s.call_start_ms)
}

pub(crate) fn span_end(s: &SpanBuilder) -> Option<u64> {
    s.hook_end_ms.or(s.result_end_ms)
}

pub(crate) fn assign_parents(spans: &mut [SpanBuilder]) {
    // Sort: start ASC, end DESC so outer spans appear before inner spans.
    spans.sort_by(|a, b| {
        let sa = span_start(a).unwrap_or(u64::MAX);
        let sb = span_start(b).unwrap_or(u64::MAX);
        sa.cmp(&sb).then_with(|| {
            let ea = span_end(a).unwrap_or(0);
            let eb = span_end(b).unwrap_or(0);
            eb.cmp(&ea)
        })
    });
    for i in 0..spans.len() {
        let (s_start, s_end) = match (span_start(&spans[i]), span_end(&spans[i])) {
            (Some(s), Some(e)) => (s, e),
            _ => continue,
        };
        let mut best: Option<(usize, u32)> = None;
        for (j, candidate) in spans[..i].iter().enumerate() {
            let (p_start, p_end) = match (span_start(candidate), span_end(candidate)) {
                (Some(s), Some(e)) => (s, e),
                _ => continue,
            };
            if p_start <= s_start && s_end <= p_end {
                let d = candidate.depth;
                if best.is_none_or(|(_, bd)| d > bd) {
                    best = Some((j, d));
                }
            }
        }
        if let Some((pi, pd)) = best {
            let pid = spans[pi].span_id.clone();
            spans[i].parent_span_id = Some(pid);
            spans[i].depth = pd + 1;
        }
    }
}

pub(crate) fn compute_subtree_costs(spans: &mut [SpanBuilder]) {
    // Seed each span's subtree with its own cost/tokens.
    for s in spans.iter_mut() {
        s.subtree_cost_usd_e6 = s.cost_usd_e6;
        s.subtree_token_count = s.tokens_in.map(|i| i + s.tokens_out.unwrap_or(0));
    }
    // Build an index: span_id → index in vec.
    let ids: Vec<String> = spans.iter().map(|s| s.span_id.clone()).collect();
    // Bottom-up: iterate in reverse depth order (deepest first).
    let order: Vec<usize> = {
        let mut v: Vec<usize> = (0..spans.len()).collect();
        v.sort_by_key(|&i| u32::MAX - spans[i].depth);
        v
    };
    for i in order {
        let (cost, tokens, pid) = (
            spans[i].subtree_cost_usd_e6,
            spans[i].subtree_token_count,
            spans[i].parent_span_id.clone(),
        );
        let Some(parent_id) = pid else { continue };
        let Some(pi) = ids.iter().position(|id| id == &parent_id) else {
            continue;
        };
        if let Some(c) = cost {
            spans[pi].subtree_cost_usd_e6 = Some(spans[pi].subtree_cost_usd_e6.unwrap_or(0) + c);
        }
        if let Some(t) = tokens {
            spans[pi].subtree_token_count = Some(spans[pi].subtree_token_count.unwrap_or(0) + t);
        }
    }
}

pub(crate) fn clear_session_spans(conn: &Connection, session_id: &str) -> Result<()> {
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

pub(crate) fn final_span_records(events: &[Event]) -> Vec<ToolSpanRecord> {
    let mut spans = build_spans(events);
    assign_parents(&mut spans);
    compute_subtree_costs(&mut spans);
    spans.iter().map(ToolSpanRecord::from_builder).collect()
}

pub(crate) fn build_spans(events: &[Event]) -> Vec<SpanBuilder> {
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

pub(crate) fn handle_tool_call(
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

pub(crate) fn handle_tool_result(
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

pub(crate) fn handle_hook(
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
    })?;
    Ok(rows.filter_map(|row| row.ok()).collect())
}

pub(crate) fn match_span_id(
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

pub(crate) fn find_open_without_call(
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

pub(crate) fn find_open_same_tool(
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

pub(crate) fn synthetic_span_id(event: &Event) -> String {
    format!("{}:{}:{}", event.session_id, event.seq, event.ts_ms)
}

pub(crate) fn hook_kind(payload: &serde_json::Value) -> Option<&'static str> {
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

pub(crate) fn hook_tool(payload: &serde_json::Value) -> Option<String> {
    ["tool_name", "tool", "name"]
        .iter()
        .find_map(|k| payload.get(k).and_then(|v| v.as_str()))
        .map(ToOwned::to_owned)
}

pub(crate) fn pick_u32(current: Option<u32>, next: Option<u32>) -> Option<u32> {
    next.or(current)
}

pub(crate) fn pick_i64(current: Option<i64>, next: Option<i64>) -> Option<i64> {
    next.or(current)
}

impl ToolSpanRecord {
    pub(crate) fn from_builder(span: &SpanBuilder) -> Self {
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
        let started = span_start(span);
        let ended = span_end(span);
        let status = if started.is_some() && ended.is_some() {
            "done"
        } else {
            "orphaned"
        };
        Self {
            span_id: span.span_id.clone(),
            session_id: span.session_id.clone(),
            tool: span.tool.clone(),
            tool_call_id: span.tool_call_id.clone(),
            status: status.to_string(),
            started_at_ms: started,
            ended_at_ms: ended,
            lead_time_ms: lead,
            tokens_in: span.tokens_in,
            tokens_out: span.tokens_out,
            reasoning_tokens: span.reasoning_tokens,
            cost_usd_e6: span.cost_usd_e6,
            paths: span.paths.iter().cloned().collect(),
            parent_span_id: span.parent_span_id.clone(),
            depth: span.depth,
            subtree_cost_usd_e6: span.subtree_cost_usd_e6,
            subtree_token_count: span.subtree_token_count,
        }
    }
}

pub(crate) fn upsert_tool_span_record(conn: &Connection, span: &ToolSpanRecord) -> Result<()> {
    conn.execute(
        "INSERT INTO tool_spans (
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
            subtree_token_count=excluded.subtree_token_count",
        params![
            &span.span_id,
            &span.session_id,
            span.tool.as_deref(),
            span.tool_call_id.as_deref(),
            &span.status,
            span.started_at_ms.map(|v| v as i64),
            span.ended_at_ms.map(|v| v as i64),
            span.lead_time_ms.map(|v| v as i64),
            span.tokens_in.map(|v| v as i64),
            span.tokens_out.map(|v| v as i64),
            span.reasoning_tokens.map(|v| v as i64),
            span.cost_usd_e6,
            serde_json::to_string(&span.paths)?,
            span.parent_span_id.as_deref(),
            span.depth as i64,
            span.subtree_cost_usd_e6,
            span.subtree_token_count.map(|v| v as i64),
        ],
    )?;
    conn.execute(
        "DELETE FROM tool_span_paths WHERE span_id = ?1",
        params![&span.span_id],
    )?;
    for path in &span.paths {
        conn.execute(
            "INSERT INTO tool_span_paths (span_id, path) VALUES (?1, ?2)",
            params![&span.span_id, path],
        )?;
    }
    Ok(())
}
