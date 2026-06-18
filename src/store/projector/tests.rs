// SPDX-License-Identifier: AGPL-3.0-or-later

use super::*;
use crate::core::event::{EventKind, EventSource};
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
            ProjectorEvent::SpanClosed(span, _) => Some(*span),
            _ => None,
        })
        .collect()
}

fn identified_event(session: &str, seq: u64, kind: EventKind, tool: &str) -> Event {
    let mut event = event(seq, seq * 10, kind, Some(tool));
    event.session_id = session.into();
    event.tool_call_id = Some("call_1".into());
    event
}

fn closed_attribution(events: Vec<ProjectorEvent>) -> Option<(String, Option<String>)> {
    span_closed(events)
        .into_iter()
        .next()
        .map(|span| (span.session_id, span.tool))
}

fn call_event(session: &str, seq: u64, tool: &str) -> Event {
    identified_event(session, seq, EventKind::ToolCall, tool)
}

fn result_event(session: &str, seq: u64, tool: &str) -> Event {
    identified_event(session, seq, EventKind::ToolResult, tool)
}

fn assert_attribution(events: Vec<ProjectorEvent>, session: &str, tool: &str) {
    assert_eq!(
        closed_attribution(events),
        Some((session.into(), Some(tool.into())))
    );
}

#[test]
fn same_call_id_in_interleaved_sessions_keeps_attribution() {
    let mut projector = Projector::default();
    projector.apply(&call_event("session-a", 1, "bash"));
    projector.apply(&call_event("session-b", 2, "read"));
    let a = projector.apply(&result_event("session-a", 3, "bash"));
    let b = projector.apply(&result_event("session-b", 4, "read"));
    assert_attribution(a, "session-a", "bash");
    assert_attribution(b, "session-b", "read");
}

#[test]
fn tool_call_result_without_id_closes_span() {
    let mut projector = Projector::default();
    projector.apply(&event(0, 10, EventKind::ToolCall, Some("bash")));
    let spans = span_closed(projector.apply(&event(1, 15, EventKind::ToolResult, Some("bash"))));
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
    let mut projector = Projector::default();
    projector.apply(&pre);
    let spans = span_closed(projector.apply(&post));
    assert_eq!(spans[0].tool.as_deref(), Some("Read"));
    assert_eq!(spans[0].lead_time_ms, Some(7));
}

#[test]
fn flush_session_marks_open_span_orphaned() {
    let mut projector = Projector::default();
    projector.apply(&event(0, 10, EventKind::ToolCall, Some("bash")));
    let spans = span_closed(projector.flush_session("s", 100));
    assert_eq!(spans[0].status, "orphaned");
    assert_eq!(spans[0].ended_at_ms, None);
}

#[test]
fn flush_expired_marks_old_open_span_orphaned() {
    let mut projector = Projector::default();
    projector.apply(&event(0, 10, EventKind::ToolCall, Some("bash")));
    let spans = span_closed(projector.flush_expired(20, 5));
    assert_eq!(spans[0].status, "orphaned");
}

#[test]
fn derived_rows_dedup_per_session() {
    let mut event = event(0, 10, EventKind::Message, None);
    event.payload = json!({
        "path": "src/lib.rs",
        "text": ".cursor/skills/tdd/SKILL.md .cursor/rules/style.mdc"
    });
    let mut projector = Projector::default();
    assert_eq!(projector.apply(&event).len(), 3);
    assert!(projector.apply(&event).is_empty());
}

#[test]
fn child_close_carries_parent_metadata() {
    let mut projector = Projector::default();
    projector.apply(&event(0, 0, EventKind::ToolCall, Some("parent")));
    projector.apply(&event(1, 10, EventKind::ToolCall, Some("child")));
    let out = projector.apply(&event(2, 20, EventKind::ToolResult, Some("child")));
    assert!(out.iter().any(|event| matches!(
        event,
        ProjectorEvent::SpanClosed(span, _)
            if span.parent_span_id.as_deref() == Some("s:0:0") && span.depth == 1
    )));
}

#[test]
fn reset_session_clears_accumulators() {
    let mut projector = Projector::default();
    let mut event = event(0, 10, EventKind::Message, None);
    event.payload = json!({"path": "src/lib.rs"});
    assert_eq!(projector.apply(&event).len(), 1);
    projector.reset_session("s");
    assert_eq!(projector.apply(&event).len(), 1);
}
