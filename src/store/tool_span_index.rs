// SPDX-License-Identifier: AGPL-3.0-or-later
//! Rebuild tool spans from one session event stream.

mod build;
mod hierarchy;
mod matching;
mod persistence;
mod record;

use crate::core::event::{Event, EventKind};
use std::collections::{BTreeSet, HashMap};

pub(crate) use matching::{
    find_open_same_tool, find_open_without_call, hook_kind, hook_tool, match_span_id, pick_i64,
    pick_u32, synthetic_span_id,
};
pub use persistence::rebuild_tool_spans_for_session;
pub(crate) use persistence::{clear_session_spans, upsert_tool_span_record};

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

pub(crate) fn span_start(span: &SpanBuilder) -> Option<u64> {
    span.hook_start_ms.or(span.call_start_ms)
}

pub(crate) fn span_end(span: &SpanBuilder) -> Option<u64> {
    span.hook_end_ms.or(span.result_end_ms)
}

pub(crate) fn final_span_records(events: &[Event]) -> Vec<ToolSpanRecord> {
    let mut spans = build_spans(events);
    assign_parents(&mut spans);
    compute_subtree_costs(&mut spans);
    spans.iter().map(ToolSpanRecord::from_builder).collect()
}

pub(crate) fn build_spans(events: &[Event]) -> Vec<SpanBuilder> {
    let mut spans = HashMap::new();
    let mut open_order = Vec::new();
    events
        .iter()
        .filter(|event| is_span_event(event))
        .for_each(|event| apply_event(event, &mut spans, &mut open_order));
    spans.into_values().collect()
}

fn is_span_event(event: &Event) -> bool {
    matches!(
        event.kind,
        EventKind::ToolCall | EventKind::ToolResult | EventKind::Hook
    )
}

fn apply_event(
    event: &Event,
    spans: &mut HashMap<String, SpanBuilder>,
    open_order: &mut Vec<String>,
) {
    match event.kind {
        EventKind::ToolCall => handle_tool_call(event, spans, open_order),
        EventKind::ToolResult => handle_tool_result(event, spans, open_order),
        EventKind::Hook => handle_hook(event, spans, open_order),
        _ => {}
    }
}

pub(crate) fn handle_tool_call(
    event: &Event,
    spans: &mut HashMap<String, SpanBuilder>,
    open_order: &mut Vec<String>,
) {
    build::handle_tool_call(event, spans, open_order);
}

pub(crate) fn handle_tool_result(
    event: &Event,
    spans: &mut HashMap<String, SpanBuilder>,
    open_order: &[String],
) {
    build::handle_tool_result(event, spans, open_order);
}

pub(crate) fn handle_hook(
    event: &Event,
    spans: &mut HashMap<String, SpanBuilder>,
    open_order: &mut Vec<String>,
) {
    build::handle_hook(event, spans, open_order);
}

pub(crate) fn assign_parents(spans: &mut [SpanBuilder]) {
    hierarchy::assign_parents(spans);
}

pub(crate) fn compute_subtree_costs(spans: &mut [SpanBuilder]) {
    hierarchy::compute_subtree_costs(spans);
}
