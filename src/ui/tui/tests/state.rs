// SPDX-License-Identifier: AGPL-3.0-or-later

use super::super::view::{EventView, SessionView};
use crate::core::event::{Event, EventKind, EventSource, SessionRecord, SessionStatus};

#[test]
fn session_view_clamps_cursor_and_suppresses_double_load() {
    let mut view = SessionView::new();
    view.set_viewport_height(4);
    assert_eq!(view.needed_page_offsets(4), vec![0]);
    assert!(view.request_page(0));
    assert!(!view.request_page(0));
    view.finish_page(0, vec![session("a", 3), session("b", 2)], 2);
    view.move_by(99);
    assert_eq!(view.cursor, 1);
    assert_eq!(view.selected().unwrap().id, "b");
}

#[test]
fn session_view_eviction_keeps_cursor_page() {
    let mut view = SessionView::new();
    view.page_size = 2;
    for offset in (0..=20).step_by(2) {
        view.cursor = offset;
        view.finish_page(
            offset,
            vec![session(&format!("s{offset}"), offset as u64)],
            30,
        );
    }
    assert!(view.window.contains_key(&20));
    assert!(!view.window.contains_key(&0));
}

#[test]
fn event_view_paginates_from_zero_and_resets_generation() {
    let mut view = EventView::new();
    view.page_size = 2;
    view.reset_for("s1");
    let token = view.generation();
    assert!(view.needed_after_seq(4).starts_with(&[0, 2]));
    assert!(view.request_page(0));
    assert!(!view.request_page(0));
    view.finish_page(0, vec![event("s1", 0), event("s1", 1)]);
    view.reset_for("s2");
    assert_ne!(view.generation(), token);
    assert!(view.window.is_empty());
}

pub(super) fn session(id: &str, started_at_ms: u64) -> SessionRecord {
    SessionRecord {
        id: id.to_string(),
        agent: "cursor".to_string(),
        model: None,
        workspace: "/ws".to_string(),
        started_at_ms,
        ended_at_ms: None,
        status: SessionStatus::Done,
        trace_path: "/trace".to_string(),
        start_commit: None,
        end_commit: None,
        branch: None,
        dirty_start: None,
        dirty_end: None,
        repo_binding_source: None,
        prompt_fingerprint: None,
        parent_session_id: None,
        agent_version: None,
        os: None,
        arch: None,
        repo_file_count: None,
        repo_total_loc: None,
    }
}

pub(super) fn event(session_id: &str, seq: u64) -> Event {
    Event {
        session_id: session_id.to_string(),
        seq,
        ts_ms: seq,
        ts_exact: true,
        kind: EventKind::ToolCall,
        source: EventSource::Tail,
        tool: None,
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
        payload: serde_json::Value::Null,
    }
}
