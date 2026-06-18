// SPDX-License-Identifier: AGPL-3.0-or-later

use super::SpanBuilder;
use super::matching::{
    find_open_same_tool, find_open_without_call, hook_kind, hook_tool, match_span_id, pick_i64,
    pick_u32, synthetic_span_id,
};
use crate::core::event::Event;
use crate::store::event_index::paths_from_event_payload;
use std::collections::HashMap;

pub(crate) fn handle_tool_call(
    event: &Event,
    spans: &mut HashMap<String, SpanBuilder>,
    open_order: &mut Vec<String>,
) {
    let tool = event.tool.clone();
    let span_id = tool_call_span_id(event, spans, open_order, tool.as_deref());
    let span = open_span(event, spans, &span_id, &tool);
    update_call(span, event, tool);
    track_open(open_order, &span_id);
}

fn tool_call_span_id(
    event: &Event,
    spans: &HashMap<String, SpanBuilder>,
    open_order: &[String],
    tool: Option<&str>,
) -> String {
    let existing = tool.and_then(|name| find_open_without_call(spans, open_order, name));
    event
        .tool_call_id
        .clone()
        .unwrap_or_else(|| existing.unwrap_or_else(|| synthetic_span_id(event)))
}

fn open_span<'a>(
    event: &Event,
    spans: &'a mut HashMap<String, SpanBuilder>,
    span_id: &str,
    tool: &Option<String>,
) -> &'a mut SpanBuilder {
    spans
        .entry(span_id.to_owned())
        .or_insert_with(|| new_span(event, span_id, tool.clone()))
}

fn new_span(event: &Event, span_id: &str, tool: Option<String>) -> SpanBuilder {
    SpanBuilder {
        span_id: span_id.to_owned(),
        session_id: event.session_id.clone(),
        tool,
        tool_call_id: event.tool_call_id.clone(),
        ..Default::default()
    }
}

fn update_call(span: &mut SpanBuilder, event: &Event, tool: Option<String>) {
    span.tool = tool;
    span.tool_call_id = event.tool_call_id.clone();
    span.call_start_ms = Some(event.ts_ms);
    span.call_start_exact = event.ts_exact;
    update_usage(span, event);
    span.paths.extend(paths_from_event_payload(&event.payload));
    span.has_call = true;
}

fn update_usage(span: &mut SpanBuilder, event: &Event) {
    span.tokens_in = pick_u32(span.tokens_in, event.tokens_in);
    span.tokens_out = pick_u32(span.tokens_out, event.tokens_out);
    span.reasoning_tokens = pick_u32(span.reasoning_tokens, event.reasoning_tokens);
    span.cost_usd_e6 = pick_i64(span.cost_usd_e6, event.cost_usd_e6);
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
    update_result(span, event);
}

fn update_result(span: &mut SpanBuilder, event: &Event) {
    span.result_end_ms = Some(event.ts_ms);
    span.result_end_exact = event.ts_exact;
    update_usage(span, event);
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
    let span_id = hook_span_id(event, spans, open_order, tool.as_deref());
    let span = open_span(event, spans, &span_id, &tool);
    update_hook(span, event, tool, kind);
    track_open(open_order, &span_id);
}

fn hook_span_id(
    event: &Event,
    spans: &HashMap<String, SpanBuilder>,
    open_order: &[String],
    tool: Option<&str>,
) -> String {
    event
        .tool_call_id
        .clone()
        .or_else(|| tool.and_then(|name| find_open_same_tool(spans, open_order, name)))
        .unwrap_or_else(|| synthetic_span_id(event))
}

fn update_hook(span: &mut SpanBuilder, event: &Event, tool: Option<String>, kind: &str) {
    merge_hook_identity(span, event, tool);
    span.paths.extend(paths_from_event_payload(&event.payload));
    match kind {
        "pre" => span.hook_start_ms = Some(event.ts_ms),
        "post" => finish_hook(span, event.ts_ms),
        _ => {}
    }
}

fn merge_hook_identity(span: &mut SpanBuilder, event: &Event, tool: Option<String>) {
    span.tool = span.tool.clone().or(tool);
    span.tool_call_id = span.tool_call_id.clone().or(event.tool_call_id.clone());
}

fn finish_hook(span: &mut SpanBuilder, end_ms: u64) {
    span.hook_end_ms = Some(end_ms);
    span.has_end = true;
}

fn track_open(open_order: &mut Vec<String>, span_id: &str) {
    if !open_order.iter().any(|id| id == span_id) {
        open_order.push(span_id.to_owned());
    }
}
