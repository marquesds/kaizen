// SPDX-License-Identifier: AGPL-3.0-or-later

use super::{Projector, ProjectorEvent};
use crate::core::event::{Event, EventKind};
use crate::store::event_index::paths_from_event_payload;
use crate::store::projector_hierarchy::open_parent;
use crate::store::tool_span_index::{
    SpanBuilder, find_open_same_tool, find_open_without_call, hook_kind, hook_tool, match_span_id,
    pick_i64, pick_u32, synthetic_span_id,
};

impl Projector {
    pub(super) fn apply_span_event(&mut self, event: &Event, out: &mut Vec<ProjectorEvent>) {
        match event.kind {
            EventKind::ToolCall => self.apply_tool_call(event),
            EventKind::ToolResult => out.extend(self.apply_tool_result(event)),
            EventKind::Hook => out.extend(self.apply_hook(event)),
            _ => {}
        }
    }

    fn apply_tool_call(&mut self, event: &Event) {
        let tool = event.tool.clone();
        let span_id = self.tool_call_span_id(event, tool.as_deref());
        let hierarchy = self.open_hierarchy(event, &span_id);
        let span = self.open_span(event, &span_id, tool.clone(), hierarchy);
        update_call(span, event, tool);
        self.push_open_order(&event.session_id, &span_id);
    }

    fn tool_call_span_id(&self, event: &Event, tool: Option<&str>) -> String {
        let existing = self.open_spans.get(&event.session_id).and_then(|spans| {
            tool.and_then(|name| find_open_without_call(spans, self.order(event), name))
        });
        event
            .tool_call_id
            .clone()
            .unwrap_or_else(|| existing.unwrap_or_else(|| synthetic_span_id(event)))
    }

    fn apply_tool_result(&mut self, event: &Event) -> Vec<ProjectorEvent> {
        let Some(span_id) = self.matched_span_id(event) else {
            return Vec::new();
        };
        let Some(span) = self.open_span_mut(event, &span_id) else {
            return Vec::new();
        };
        update_result(span, event);
        self.remove_open(&event.session_id, &span_id)
            .map(|span| self.close_span(span))
            .unwrap_or_default()
    }

    fn apply_hook(&mut self, event: &Event) -> Vec<ProjectorEvent> {
        let Some(kind) = hook_kind(&event.payload) else {
            return Vec::new();
        };
        let tool = hook_tool(&event.payload);
        let span_id = self.hook_span_id(event, tool.as_deref());
        let hierarchy = self.open_hierarchy(event, &span_id);
        update_hook(
            self.open_span(event, &span_id, tool.clone(), hierarchy),
            event,
            tool,
            kind,
        );
        self.push_open_order(&event.session_id, &span_id);
        self.close_post_hook(event, &span_id, kind)
    }

    fn hook_span_id(&self, event: &Event, tool: Option<&str>) -> String {
        let matching_tool = self.open_spans.get(&event.session_id).and_then(|spans| {
            tool.and_then(|name| find_open_same_tool(spans, self.order(event), name))
        });
        event
            .tool_call_id
            .clone()
            .or(matching_tool)
            .unwrap_or_else(|| synthetic_span_id(event))
    }

    fn matched_span_id(&self, event: &Event) -> Option<String> {
        let spans = self.open_spans.get(&event.session_id)?;
        match_span_id(event, spans, self.order(event))
    }

    fn open_hierarchy(&self, event: &Event, span_id: &str) -> (Option<String>, u32) {
        self.open_spans
            .get(&event.session_id)
            .map(|spans| open_parent(self.order(event), spans, span_id))
            .unwrap_or((None, 0))
    }

    fn open_span(
        &mut self,
        event: &Event,
        span_id: &str,
        tool: Option<String>,
        hierarchy: (Option<String>, u32),
    ) -> &mut SpanBuilder {
        self.open_spans
            .entry(event.session_id.clone())
            .or_default()
            .entry(span_id.to_owned())
            .or_insert_with(|| new_builder(event, span_id, tool, hierarchy))
    }

    fn open_span_mut(&mut self, event: &Event, span_id: &str) -> Option<&mut SpanBuilder> {
        self.open_spans.get_mut(&event.session_id)?.get_mut(span_id)
    }

    fn close_post_hook(&mut self, event: &Event, span_id: &str, kind: &str) -> Vec<ProjectorEvent> {
        if kind != "post" {
            return Vec::new();
        }
        self.remove_open(&event.session_id, span_id)
            .map(|span| self.close_span(span))
            .unwrap_or_default()
    }

    pub(super) fn order(&self, event: &Event) -> &[String] {
        self.open_order
            .get(&event.session_id)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    fn push_open_order(&mut self, session_id: &str, span_id: &str) {
        let order = self.open_order.entry(session_id.to_owned()).or_default();
        if !order.iter().any(|id| id == span_id) {
            order.push(span_id.to_owned());
        }
    }
}

fn new_builder(
    event: &Event,
    span_id: &str,
    tool: Option<String>,
    hierarchy: (Option<String>, u32),
) -> SpanBuilder {
    SpanBuilder {
        span_id: span_id.to_owned(),
        session_id: event.session_id.clone(),
        tool,
        tool_call_id: event.tool_call_id.clone(),
        parent_span_id: hierarchy.0,
        depth: hierarchy.1,
        ..Default::default()
    }
}

fn update_call(span: &mut SpanBuilder, event: &Event, tool: Option<String>) {
    span.tool = tool;
    span.tool_call_id = event.tool_call_id.clone();
    span.call_start_ms = Some(event.ts_ms);
    span.call_start_exact = event.ts_exact;
    copy_usage(span, event);
    span.paths.extend(paths_from_event_payload(&event.payload));
    span.has_call = true;
}

fn update_result(span: &mut SpanBuilder, event: &Event) {
    span.result_end_ms = Some(event.ts_ms);
    span.result_end_exact = event.ts_exact;
    copy_usage(span, event);
    span.paths.extend(paths_from_event_payload(&event.payload));
    span.has_end = true;
}

fn copy_usage(span: &mut SpanBuilder, event: &Event) {
    span.tokens_in = pick_u32(span.tokens_in, event.tokens_in);
    span.tokens_out = pick_u32(span.tokens_out, event.tokens_out);
    span.reasoning_tokens = pick_u32(span.reasoning_tokens, event.reasoning_tokens);
    span.cost_usd_e6 = pick_i64(span.cost_usd_e6, event.cost_usd_e6);
}

fn update_hook(span: &mut SpanBuilder, event: &Event, tool: Option<String>, kind: &str) {
    span.tool = span.tool.clone().or(tool);
    span.tool_call_id = span.tool_call_id.clone().or(event.tool_call_id.clone());
    span.paths.extend(paths_from_event_payload(&event.payload));
    match kind {
        "pre" => span.hook_start_ms = Some(event.ts_ms),
        "post" => span.hook_end_ms = Some(event.ts_ms),
        _ => {}
    }
    span.has_end |= kind == "post";
}
