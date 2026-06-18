// SPDX-License-Identifier: AGPL-3.0-or-later

use super::SpanBuilder;
use crate::core::event::Event;
use std::collections::HashMap;

pub(crate) fn match_span_id(
    event: &Event,
    spans: &HashMap<String, SpanBuilder>,
    open_order: &[String],
) -> Option<String> {
    matching_call_id(event, spans)
        .or_else(|| {
            event
                .tool
                .as_deref()
                .and_then(|tool| find_open_same_tool(spans, open_order, tool))
        })
        .or_else(|| open_order.last().cloned())
}

fn matching_call_id(event: &Event, spans: &HashMap<String, SpanBuilder>) -> Option<String> {
    event
        .tool_call_id
        .as_ref()
        .filter(|id| spans.contains_key(*id))
        .cloned()
}

pub(crate) fn find_open_without_call(
    spans: &HashMap<String, SpanBuilder>,
    open_order: &[String],
    tool: &str,
) -> Option<String> {
    find_open(spans, open_order, |span| {
        span.tool.as_deref() == Some(tool) && !span.has_call
    })
}

pub(crate) fn find_open_same_tool(
    spans: &HashMap<String, SpanBuilder>,
    open_order: &[String],
    tool: &str,
) -> Option<String> {
    find_open(spans, open_order, |span| {
        span.tool.as_deref() == Some(tool) && !span.has_end
    })
}

fn find_open(
    spans: &HashMap<String, SpanBuilder>,
    open_order: &[String],
    matches: impl Fn(&SpanBuilder) -> bool,
) -> Option<String> {
    open_order.iter().rev().find_map(|id| {
        spans
            .get(id)
            .and_then(|span| matches(span).then(|| id.clone()))
    })
}

pub(crate) fn synthetic_span_id(event: &Event) -> String {
    format!("{}:{}:{}", event.session_id, event.seq, event.ts_ms)
}

pub(crate) fn hook_kind(payload: &serde_json::Value) -> Option<&'static str> {
    let raw = payload
        .get("event")
        .and_then(|value| value.as_str())
        .or_else(|| {
            payload
                .get("hook_event_name")
                .and_then(|value| value.as_str())
        })?;
    match raw {
        "PreToolUse" | "pre_tool_use" => Some("pre"),
        "PostToolUse" | "post_tool_use" => Some("post"),
        _ => None,
    }
}

pub(crate) fn hook_tool(payload: &serde_json::Value) -> Option<String> {
    ["tool_name", "tool", "name"]
        .iter()
        .find_map(|key| payload.get(key).and_then(|value| value.as_str()))
        .map(ToOwned::to_owned)
}

pub(crate) fn pick_u32(current: Option<u32>, next: Option<u32>) -> Option<u32> {
    next.or(current)
}

pub(crate) fn pick_i64(current: Option<i64>, next: Option<i64>) -> Option<i64> {
    next.or(current)
}
