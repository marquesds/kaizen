// SPDX-License-Identifier: AGPL-3.0-or-later

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
fn nested_tool_pairs_match_legacy() {
    let events = vec![
        with_call_id(
            event("nested", 0, 10, EventKind::ToolCall, "bash"),
            "parent",
        ),
        with_call_id(event("nested", 1, 20, EventKind::ToolCall, "read"), "child"),
        with_call_id(
            event("nested", 2, 30, EventKind::ToolResult, "read"),
            "child",
        ),
        with_call_id(
            event("nested", 3, 40, EventKind::ToolResult, "bash"),
            "parent",
        ),
    ];
    assert_legacy_incremental_parity(&events);
}

fn with_call_id(mut event: Event, id: &str) -> Event {
    event.tool_call_id = Some(id.to_string());
    event
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
