// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::core::event::{Event, SessionRecord, SessionStatus};
use crate::store::span_tree::SpanNode;
use ratatui::text::{Line, Span};
use std::collections::HashMap;
use time::OffsetDateTime;

const THIRTY_DAYS_SEC: u64 = 30 * 24 * 3600;
const MS_HEURISTIC_THRESHOLD: u64 = 1_000_000_000_000;

pub(super) fn time_ago_label(now_ms: u64, ts_ms: u64) -> String {
    if ts_ms == 0 {
        return "?".to_string();
    }
    let ts = normalize_timestamp(now_ms, ts_ms);
    let diff_sec = now_ms.saturating_sub(ts) / 1000;
    if diff_sec > THIRTY_DAYS_SEC {
        return abs_ts_label(ts);
    }
    relative_time_label(diff_sec)
}

fn normalize_timestamp(now_ms: u64, ts_ms: u64) -> u64 {
    if ts_ms < MS_HEURISTIC_THRESHOLD && now_ms >= MS_HEURISTIC_THRESHOLD {
        ts_ms.saturating_mul(1000)
    } else {
        ts_ms
    }
}

fn relative_time_label(diff_sec: u64) -> String {
    match diff_sec {
        0 => "just now".to_string(),
        seconds if seconds < 60 => format!("{seconds}s"),
        seconds if seconds < 3600 => format!("{}m", seconds / 60),
        seconds if seconds < 86_400 => format!("{}h", seconds / 3600),
        seconds => format!("{}d", seconds / 86_400),
    }
}

fn abs_ts_label(ts_ms: u64) -> String {
    let Ok(dt) = OffsetDateTime::from_unix_timestamp((ts_ms / 1000) as i64) else {
        return "?".to_string();
    };
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}",
        dt.year(),
        u8::from(dt.month()),
        dt.day(),
        dt.hour(),
        dt.minute()
    )
}

pub(super) fn truncate(value: &str, max: usize) -> &str {
    if value.chars().count() <= max {
        return value;
    }
    value
        .char_indices()
        .nth(max.saturating_sub(1))
        .map(|(index, _)| &value[..index])
        .unwrap_or(value)
}

pub(super) fn model_suffix(model: &Option<String>) -> String {
    match model {
        Some(model) if !model.is_empty() => format!(" {}", truncate(model, 20)),
        _ => " —".to_string(),
    }
}

pub(super) fn session_status_letter(session: &SessionRecord) -> char {
    match session.status {
        SessionStatus::Running => 'R',
        SessionStatus::Waiting => 'W',
        SessionStatus::Idle => 'I',
        SessionStatus::Done => 'D',
    }
}

fn format_event_tokens(event: &Event) -> Option<String> {
    let mut output = match (event.tokens_in, event.tokens_out) {
        (Some(input), Some(output)) => format!("{input}/{output}"),
        (Some(input), None) => input.to_string(),
        (None, Some(output)) => output.to_string(),
        (None, None) => String::new(),
    };
    if let Some(reasoning) = event.reasoning_tokens {
        output = reasoning_tokens(&output, reasoning);
    }
    (!output.is_empty()).then_some(output)
}

fn reasoning_tokens(tokens: &str, reasoning: u32) -> String {
    if tokens.is_empty() {
        format!("r{reasoning}")
    } else {
        format!("{tokens}+r{reasoning}")
    }
}

pub(super) fn event_row_text(now_ms: u64, event: &Event, lead: &HashMap<String, u64>) -> String {
    let age = time_ago_label(now_ms, event.ts_ms);
    let tool = event.tool.as_deref().unwrap_or("-");
    let lead = event_lead(event, lead);
    let tokens = format_event_tokens(event)
        .map(|tokens| format!(" tok={tokens}"))
        .unwrap_or_default();
    format!("{age}  {kind:?}  {tool}{tokens}  {lead}", kind = event.kind)
}

fn event_lead(event: &Event, lead: &HashMap<String, u64>) -> String {
    event
        .tool_call_id
        .as_ref()
        .and_then(|id| lead.get(id).copied())
        .map(|milliseconds| format!("{milliseconds}ms"))
        .unwrap_or_else(|| "—".to_string())
}

pub(super) fn span_depth_lines(nodes: &[SpanNode]) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    nodes
        .iter()
        .for_each(|node| push_span_line(&mut lines, node, 0));
    lines
}

fn push_span_line(lines: &mut Vec<Line<'static>>, node: &SpanNode, depth: u32) {
    let indent = "  ".repeat(depth as usize);
    let prefix = if depth == 0 { "┌ " } else { "├ " };
    let cost = node
        .span
        .subtree_cost_usd_e6
        .map(|value| format!(" ${:.4}", value as f64 / 1_000_000.0))
        .unwrap_or_default();
    lines.push(Line::from(Span::raw(format!(
        "{}{}{}{}",
        indent, prefix, node.span.tool, cost
    ))));
    node.children
        .iter()
        .for_each(|child| push_span_line(lines, child, depth + 1));
}

pub(super) fn event_detail_text(event: &Event, lead: &HashMap<String, u64>) -> String {
    let head = format!(
        "Event {}\nType: {}\nTool: {}\nCall: {}\nInput tokens: {}\nOutput tokens: {}\nReasoning tokens: {}\nCost: {}\nLead time: {}\n\nPayload\n",
        event.seq,
        event_kind_label(&event.kind),
        event.tool.as_deref().unwrap_or("-"),
        event.tool_call_id.as_deref().unwrap_or("—"),
        optional_u32(event.tokens_in),
        optional_u32(event.tokens_out),
        optional_u32(event.reasoning_tokens),
        event_cost(event.cost_usd_e6),
        event_lead(event, lead)
    );
    let json =
        serde_json::to_string_pretty(&event.payload).unwrap_or_else(|_| event.payload.to_string());
    head + &json
}

fn event_kind_label(kind: &crate::core::event::EventKind) -> &'static str {
    use crate::core::event::EventKind::*;
    match kind {
        ToolCall => "tool call",
        ToolResult => "tool result",
        Message => "message",
        Error => "error",
        Cost => "cost",
        Hook => "hook",
        Lifecycle => "lifecycle",
    }
}

fn optional_u32(value: Option<u32>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "—".into())
}

fn event_cost(value: Option<i64>) -> String {
    value
        .map(|value| format!("${:.6}", value as f64 / 1_000_000.0))
        .unwrap_or_else(|| "—".into())
}

pub(super) fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
