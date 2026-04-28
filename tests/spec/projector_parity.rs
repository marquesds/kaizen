// SPDX-License-Identifier: AGPL-3.0-or-later
use kaizen::core::event::{Event, EventKind, EventSource, SessionRecord, SessionStatus};
use kaizen::store::Store;
use proptest::prelude::*;
use serde_json::json;
use std::sync::Mutex;

static ENV_LOCK: Mutex<()> = Mutex::new(());

fn session(id: &str) -> SessionRecord {
    SessionRecord {
        id: id.to_string(),
        agent: "codex".into(),
        model: Some("gpt".into()),
        workspace: "/ws".into(),
        started_at_ms: 0,
        ended_at_ms: None,
        status: SessionStatus::Running,
        trace_path: String::new(),
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

fn event(session_id: &str, seq: u64, ts_ms: u64, kind: EventKind, tool: &str) -> Event {
    Event {
        session_id: session_id.into(),
        seq,
        ts_ms,
        ts_exact: true,
        kind,
        source: EventSource::Tail,
        tool: Some(tool.into()),
        tool_call_id: Some(format!("call-{seq}")),
        tokens_in: Some(seq as u32 + 1),
        tokens_out: Some(seq as u32 + 2),
        reasoning_tokens: Some(seq as u32),
        cost_usd_e6: Some(seq as i64),
        stop_reason: None,
        latency_ms: None,
        ttft_ms: None,
        retry_count: None,
        context_used_tokens: None,
        context_max_tokens: None,
        cache_creation_tokens: None,
        cache_read_tokens: None,
        system_prompt_tokens: None,
        payload: json!({"input": {"path": format!("src/{seq}.rs")}}),
    }
}

fn paired_events(session_id: &str, pairs: usize) -> Vec<Event> {
    let mut out = Vec::new();
    for i in 0..pairs {
        let call_seq = (i * 2) as u64;
        let result_seq = call_seq + 1;
        let mut call = event(
            session_id,
            call_seq,
            1_000 + call_seq * 10,
            EventKind::ToolCall,
            "bash",
        );
        call.tool_call_id = Some(format!("call-{i}"));
        let mut result = event(
            session_id,
            result_seq,
            1_000 + result_seq * 10,
            EventKind::ToolResult,
            "bash",
        );
        result.tool_call_id = Some(format!("call-{i}"));
        out.push(call);
        out.push(result);
    }
    out
}

fn rows(store: &Store, session_id: &str) -> Vec<String> {
    store
        .tool_spans_for_session(session_id)
        .unwrap()
        .into_iter()
        .map(|row| {
            format!(
                "{}|{}|{:?}|{:?}|{}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}",
                row.span_id,
                row.session_id,
                row.tool,
                row.tool_call_id,
                row.status,
                row.started_at_ms,
                row.ended_at_ms,
                row.lead_time_ms,
                row.tokens_in,
                row.tokens_out,
                row.reasoning_tokens,
                row.cost_usd_e6,
                row.paths
            )
        })
        .collect()
}

fn run_with_mode(events: &[Event], mode: Option<&str>) -> Vec<String> {
    let dir = tempfile::tempdir().unwrap();
    match mode {
        Some(value) => unsafe { std::env::set_var("KAIZEN_PROJECTOR", value) },
        None => unsafe { std::env::remove_var("KAIZEN_PROJECTOR") },
    }
    let store = Store::open(&dir.path().join("kaizen.db")).unwrap();
    let session_id = &events[0].session_id;
    store.upsert_session(&session(session_id)).unwrap();
    for event in events {
        store.append_event(event).unwrap();
    }
    store.flush_projector_session(session_id, 99_999).unwrap();
    rows(&store, session_id)
}

fn assert_legacy_incremental_parity(events: &[Event]) {
    let _guard = ENV_LOCK.lock().unwrap();
    let legacy = run_with_mode(events, Some("legacy"));
    let incremental = run_with_mode(events, None);
    unsafe { std::env::remove_var("KAIZEN_PROJECTOR") };
    assert_eq!(legacy, incremental);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(16))]

    #[test]
    fn generated_tool_pairs_match_legacy(pairs in 1usize..32) {
        let events = paired_events("prop", pairs);
        assert_legacy_incremental_parity(&events);
    }
}

#[test]
fn hook_fixture_shape_matches_legacy() {
    let mut pre = event("hook", 0, 10, EventKind::Hook, "Read");
    pre.tool = None;
    pre.tool_call_id = None;
    pre.payload = json!({"event": "PreToolUse", "tool_name": "Read", "path": "src/lib.rs"});
    let mut post = event("hook", 1, 20, EventKind::Hook, "Read");
    post.tool = None;
    post.tool_call_id = None;
    post.payload = json!({"event": "PostToolUse", "tool_name": "Read", "path": "src/lib.rs"});
    assert_legacy_incremental_parity(&[pre, post]);
}

#[test]
#[ignore = "requires KAIZEN_PARITY_CORPUS jsonl with serialized Event rows"]
fn real_session_corpus_matches_legacy() {
    let path = std::env::var("KAIZEN_PARITY_CORPUS").expect("KAIZEN_PARITY_CORPUS");
    let raw = std::fs::read_to_string(path).unwrap();
    let mut sessions: std::collections::BTreeMap<String, Vec<Event>> = Default::default();
    for line in raw.lines().filter(|line| !line.trim().is_empty()) {
        let event: Event = serde_json::from_str(line).unwrap();
        sessions
            .entry(event.session_id.clone())
            .or_default()
            .push(event);
    }
    for events in sessions.values().take(1_000) {
        assert_legacy_incremental_parity(events);
    }
}
