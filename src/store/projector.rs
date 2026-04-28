// SPDX-License-Identifier: AGPL-3.0-or-later
//! Incremental projector for event-derived rows.

use crate::core::event::{Event, EventKind};
use crate::metrics::types::ToolSpanSample;
use crate::store::event_index::{
    paths_from_event_payload, rules_from_event_json, skills_from_event_json,
};
use crate::store::tool_span_index::{
    SpanBuilder, ToolSpanRecord, find_open_same_tool, find_open_without_call, hook_kind, hook_tool,
    match_span_id, pick_i64, pick_u32, span_start, synthetic_span_id,
};
use std::collections::{BTreeMap, HashMap, HashSet};

pub const DEFAULT_ORPHAN_TTL_MS: u64 = 60 * 60 * 1_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenSpan {
    pub(crate) inner: SpanBuilder,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosedSpan {
    pub record: ToolSpanRecord,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectorEvent {
    SpanClosed(ToolSpanRecord, ToolSpanSample),
    SpanPatched(ToolSpanRecord),
    FileTouched { session: String, path: String },
    SkillUsed { session: String, skill: String },
    RuleUsed { session: String, rule: String },
}

#[derive(Debug, Default)]
pub struct Projector {
    open_spans: HashMap<String, SpanBuilder>,
    open_order: HashMap<String, Vec<String>>,
    closed_spans: HashMap<String, BTreeMap<String, ToolSpanRecord>>,
    file_touch: HashMap<String, HashSet<String>>,
    skill_use: HashMap<String, HashSet<String>>,
    rule_use: HashMap<String, HashSet<String>>,
    last_seq: HashMap<String, u64>,
}

impl Projector {
    pub fn apply(&mut self, evt: &Event) -> Vec<ProjectorEvent> {
        let mut out = self.apply_derived(evt);
        if !matches!(
            evt.kind,
            EventKind::ToolCall | EventKind::ToolResult | EventKind::Hook
        ) {
            self.last_seq.insert(evt.session_id.clone(), evt.seq);
            return out;
        }
        match evt.kind {
            EventKind::ToolCall => self.apply_tool_call(evt),
            EventKind::ToolResult => out.extend(self.apply_tool_result(evt)),
            EventKind::Hook => out.extend(self.apply_hook(evt)),
            _ => {}
        }
        self.last_seq.insert(evt.session_id.clone(), evt.seq);
        out
    }

    pub fn flush_session(&mut self, session_id: &str, _now_ms: u64) -> Vec<ProjectorEvent> {
        let ids = self
            .open_order
            .remove(session_id)
            .unwrap_or_default()
            .into_iter()
            .filter(|id| self.open_spans.contains_key(id))
            .collect::<Vec<_>>();
        let mut out = Vec::new();
        for id in ids {
            if let Some(span) = self.open_spans.remove(&id) {
                out.extend(self.close_span(span));
            }
        }
        out
    }

    pub fn flush_expired(&mut self, now_ms: u64, ttl_ms: u64) -> Vec<ProjectorEvent> {
        let expired = self
            .open_spans
            .iter()
            .filter_map(|(id, span)| {
                let started = span_start(span)?;
                (now_ms.saturating_sub(started) > ttl_ms).then(|| id.clone())
            })
            .collect::<Vec<_>>();
        let mut out = Vec::new();
        for id in expired {
            let Some(span) = self.open_spans.remove(&id) else {
                continue;
            };
            if let Some(order) = self.open_order.get_mut(&span.session_id) {
                order.retain(|open_id| open_id != &id);
            }
            out.extend(self.close_span(span));
        }
        out
    }

    pub fn reset_session(&mut self, session_id: &str) {
        if let Some(ids) = self.open_order.remove(session_id) {
            for id in ids {
                self.open_spans.remove(&id);
            }
        }
        self.closed_spans.remove(session_id);
        self.file_touch.remove(session_id);
        self.skill_use.remove(session_id);
        self.rule_use.remove(session_id);
        self.last_seq.remove(session_id);
    }

    pub fn last_seq(&self, session_id: &str) -> Option<u64> {
        self.last_seq.get(session_id).copied()
    }

    fn apply_derived(&mut self, evt: &Event) -> Vec<ProjectorEvent> {
        let mut out = Vec::new();
        let session = &evt.session_id;
        for path in paths_from_event_payload(&evt.payload) {
            if self
                .file_touch
                .entry(session.clone())
                .or_default()
                .insert(path.clone())
            {
                out.push(ProjectorEvent::FileTouched {
                    session: session.clone(),
                    path,
                });
            }
        }
        for skill in skills_from_event_json(&evt.payload) {
            if self
                .skill_use
                .entry(session.clone())
                .or_default()
                .insert(skill.clone())
            {
                out.push(ProjectorEvent::SkillUsed {
                    session: session.clone(),
                    skill,
                });
            }
        }
        for rule in rules_from_event_json(&evt.payload) {
            if self
                .rule_use
                .entry(session.clone())
                .or_default()
                .insert(rule.clone())
            {
                out.push(ProjectorEvent::RuleUsed {
                    session: session.clone(),
                    rule,
                });
            }
        }
        out
    }

    fn apply_tool_call(&mut self, event: &Event) {
        let tool = event.tool.clone();
        let existing = tool
            .as_deref()
            .and_then(|name| find_open_without_call(&self.open_spans, self.order(event), name));
        let span_id = event
            .tool_call_id
            .clone()
            .unwrap_or_else(|| existing.unwrap_or_else(|| synthetic_span_id(event)));
        let span = self
            .open_spans
            .entry(span_id.clone())
            .or_insert_with(|| SpanBuilder {
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
        self.push_open_order(&event.session_id, &span_id);
    }

    fn apply_tool_result(&mut self, event: &Event) -> Vec<ProjectorEvent> {
        let Some(span_id) = match_span_id(event, &self.open_spans, self.order(event)) else {
            return Vec::new();
        };
        let Some(span) = self.open_spans.get_mut(&span_id) else {
            return Vec::new();
        };
        span.result_end_ms = Some(event.ts_ms);
        span.result_end_exact = event.ts_exact;
        span.tokens_in = pick_u32(span.tokens_in, event.tokens_in);
        span.tokens_out = pick_u32(span.tokens_out, event.tokens_out);
        span.reasoning_tokens = pick_u32(span.reasoning_tokens, event.reasoning_tokens);
        span.cost_usd_e6 = pick_i64(span.cost_usd_e6, event.cost_usd_e6);
        span.paths.extend(paths_from_event_payload(&event.payload));
        span.has_end = true;
        self.remove_open(&event.session_id, &span_id)
            .map(|span| self.close_span(span))
            .unwrap_or_default()
    }

    fn apply_hook(&mut self, event: &Event) -> Vec<ProjectorEvent> {
        let Some(kind) = hook_kind(&event.payload) else {
            return Vec::new();
        };
        let tool = hook_tool(&event.payload);
        let span_id = event
            .tool_call_id
            .clone()
            .or_else(|| {
                tool.as_deref()
                    .and_then(|name| find_open_same_tool(&self.open_spans, self.order(event), name))
            })
            .unwrap_or_else(|| synthetic_span_id(event));
        let span = self
            .open_spans
            .entry(span_id.clone())
            .or_insert_with(|| SpanBuilder {
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
        self.push_open_order(&event.session_id, &span_id);
        if kind == "post" {
            return self
                .remove_open(&event.session_id, &span_id)
                .map(|span| self.close_span(span))
                .unwrap_or_default();
        }
        Vec::new()
    }

    fn close_span(&mut self, mut span: SpanBuilder) -> Vec<ProjectorEvent> {
        let session_id = span.session_id.clone();
        span.parent_span_id = None;
        span.depth = 0;
        span.subtree_cost_usd_e6 = span.cost_usd_e6;
        span.subtree_token_count = span.tokens_in.map(|i| i + span.tokens_out.unwrap_or(0));
        let mut record = ToolSpanRecord::from_builder(&span);
        let before = self
            .closed_spans
            .get(&session_id)
            .cloned()
            .unwrap_or_default();
        self.closed_spans
            .entry(session_id.clone())
            .or_default()
            .insert(record.span_id.clone(), record.clone());
        self.recompute_session_tree(&session_id);
        record = self
            .closed_spans
            .get(&session_id)
            .and_then(|spans| spans.get(&record.span_id))
            .cloned()
            .unwrap_or(record);
        let sample = ToolSpanSample::from(&record);
        let mut out = vec![ProjectorEvent::SpanClosed(record.clone(), sample)];
        if let Some(after) = self.closed_spans.get(&session_id) {
            for (id, span) in after {
                if id == &record.span_id {
                    continue;
                }
                if before.get(id) != Some(span) {
                    out.push(ProjectorEvent::SpanPatched(span.clone()));
                }
            }
        }
        out
    }

    fn recompute_session_tree(&mut self, session_id: &str) {
        let Some(map) = self.closed_spans.get_mut(session_id) else {
            return;
        };
        let mut spans = map
            .values()
            .map(record_to_builder)
            .collect::<Vec<SpanBuilder>>();
        crate::store::tool_span_index::assign_parents(&mut spans);
        crate::store::tool_span_index::compute_subtree_costs(&mut spans);
        map.clear();
        for span in spans {
            let record = ToolSpanRecord::from_builder(&span);
            map.insert(record.span_id.clone(), record);
        }
    }

    fn order(&self, event: &Event) -> &[String] {
        self.open_order
            .get(&event.session_id)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    fn push_open_order(&mut self, session_id: &str, span_id: &str) {
        let order = self.open_order.entry(session_id.to_string()).or_default();
        if !order.iter().any(|id| id == span_id) {
            order.push(span_id.to_string());
        }
    }

    fn remove_open(&mut self, session_id: &str, span_id: &str) -> Option<SpanBuilder> {
        if let Some(order) = self.open_order.get_mut(session_id) {
            order.retain(|id| id != span_id);
        }
        self.open_spans.remove(span_id)
    }
}

fn record_to_builder(record: &ToolSpanRecord) -> SpanBuilder {
    let paths = record.paths.iter().cloned().collect();
    SpanBuilder {
        span_id: record.span_id.clone(),
        session_id: record.session_id.clone(),
        tool: record.tool.clone(),
        tool_call_id: record.tool_call_id.clone(),
        hook_start_ms: record.started_at_ms,
        hook_end_ms: None,
        call_start_ms: record.started_at_ms,
        result_end_ms: record.ended_at_ms,
        call_start_exact: record.lead_time_ms.is_some(),
        result_end_exact: record.lead_time_ms.is_some(),
        tokens_in: record.tokens_in,
        tokens_out: record.tokens_out,
        reasoning_tokens: record.reasoning_tokens,
        cost_usd_e6: record.cost_usd_e6,
        paths,
        has_call: record.started_at_ms.is_some(),
        has_end: record.ended_at_ms.is_some(),
        parent_span_id: None,
        depth: 0,
        subtree_cost_usd_e6: None,
        subtree_token_count: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::event::EventSource;
    use serde_json::json;

    fn event(seq: u64, ts_ms: u64, kind: EventKind, tool: Option<&str>) -> Event {
        Event {
            session_id: "s".into(),
            seq,
            ts_ms,
            ts_exact: true,
            kind,
            source: EventSource::Tail,
            tool: tool.map(str::to_string),
            tool_call_id: None,
            tokens_in: None,
            tokens_out: None,
            reasoning_tokens: None,
            cost_usd_e6: None,
            stop_reason: None,
            latency_ms: None,
            ttft_ms: None,
            retry_count: None,
            context_used_tokens: None,
            context_max_tokens: None,
            cache_creation_tokens: None,
            cache_read_tokens: None,
            system_prompt_tokens: None,
            payload: json!({}),
        }
    }

    fn span_closed(events: Vec<ProjectorEvent>) -> Vec<ToolSpanRecord> {
        events
            .into_iter()
            .filter_map(|event| match event {
                ProjectorEvent::SpanClosed(span, _) => Some(span),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn tool_call_result_without_id_closes_span() {
        let mut p = Projector::default();
        p.apply(&event(0, 10, EventKind::ToolCall, Some("bash")));
        let spans = span_closed(p.apply(&event(1, 15, EventKind::ToolResult, Some("bash"))));
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].status, "done");
        assert_eq!(spans[0].lead_time_ms, Some(5));
    }

    #[test]
    fn hook_pre_post_matching_closes_span() {
        let mut pre = event(0, 10, EventKind::Hook, None);
        pre.payload = json!({"event": "PreToolUse", "tool_name": "Read"});
        let mut post = event(1, 17, EventKind::Hook, None);
        post.payload = json!({"event": "PostToolUse", "tool_name": "Read"});
        let mut p = Projector::default();
        p.apply(&pre);
        let spans = span_closed(p.apply(&post));
        assert_eq!(spans[0].tool.as_deref(), Some("Read"));
        assert_eq!(spans[0].lead_time_ms, Some(7));
    }

    #[test]
    fn flush_session_marks_open_span_orphaned() {
        let mut p = Projector::default();
        p.apply(&event(0, 10, EventKind::ToolCall, Some("bash")));
        let spans = span_closed(p.flush_session("s", 100));
        assert_eq!(spans[0].status, "orphaned");
        assert_eq!(spans[0].ended_at_ms, None);
    }

    #[test]
    fn flush_expired_marks_old_open_span_orphaned() {
        let mut p = Projector::default();
        p.apply(&event(0, 10, EventKind::ToolCall, Some("bash")));
        let spans = span_closed(p.flush_expired(20, 5));
        assert_eq!(spans[0].status, "orphaned");
    }

    #[test]
    fn derived_rows_dedup_per_session() {
        let mut e = event(0, 10, EventKind::Message, None);
        e.payload = json!({
            "path": "src/lib.rs",
            "text": ".cursor/skills/tdd/SKILL.md .cursor/rules/style.mdc"
        });
        let mut p = Projector::default();
        assert_eq!(p.apply(&e).len(), 3);
        assert!(p.apply(&e).is_empty());
    }

    #[test]
    fn parent_close_patches_existing_child() {
        let mut p = Projector::default();
        p.apply(&event(0, 0, EventKind::ToolCall, Some("parent")));
        p.apply(&event(1, 10, EventKind::ToolCall, Some("child")));
        p.apply(&event(2, 20, EventKind::ToolResult, Some("child")));
        let out = p.apply(&event(3, 30, EventKind::ToolResult, Some("parent")));
        assert!(
            out.iter()
                .any(|event| matches!(event, ProjectorEvent::SpanPatched(span) if span.depth == 1))
        );
    }

    #[test]
    fn reset_session_clears_accumulators() {
        let mut p = Projector::default();
        let mut e = event(0, 10, EventKind::Message, None);
        e.payload = json!({"path": "src/lib.rs"});
        assert_eq!(p.apply(&e).len(), 1);
        p.reset_session("s");
        assert_eq!(p.apply(&e).len(), 1);
    }
}
