use super::super::Store;
use super::{EventKind, EventSource, SessionStatus, make_event, make_session};
use serde_json::json;
use tempfile::TempDir;

#[test]
fn append_and_list_events_round_trip() {
    let dir = TempDir::new().unwrap();
    let store = Store::open(&dir.path().join("kaizen.db")).unwrap();
    let s = make_session("s2");
    store.upsert_session(&s).unwrap();
    store.append_event(&make_event("s2", 0)).unwrap();
    store.append_event(&make_event("s2", 1)).unwrap();

    let sessions = store.list_sessions("/ws").unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id, "s2");
}

#[test]
fn list_events_for_session_round_trip() {
    let dir = TempDir::new().unwrap();
    let store = Store::open(&dir.path().join("kaizen.db")).unwrap();
    store.upsert_session(&make_session("s4")).unwrap();
    store.append_event(&make_event("s4", 0)).unwrap();
    store.append_event(&make_event("s4", 1)).unwrap();
    let events = store.list_events_for_session("s4").unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].seq, 0);
    assert_eq!(events[1].seq, 1);
}

#[test]
fn legacy_hook_rows_read_as_semantic_tool_events() {
    let dir = TempDir::new().unwrap();
    let store = Store::open(&dir.path().join("kaizen.db")).unwrap();
    store.upsert_session(&make_session("legacy-hook")).unwrap();
    let mut event = make_event("legacy-hook", 0);
    event.kind = EventKind::Hook;
    event.source = EventSource::Hook;
    event.tool = None;
    event.payload = json!({"hook_event_name":"PreToolUse","tool_name":"Read"});
    store.append_event(&event).unwrap();

    let event = store
        .list_events_for_session("legacy-hook")
        .unwrap()
        .remove(0);
    assert_eq!(event.kind, EventKind::ToolCall);
    assert_eq!(event.tool.as_deref(), Some("Read"));
}

#[test]
fn list_events_page_uses_inclusive_seq_cursor() {
    let dir = TempDir::new().unwrap();
    let store = Store::open(&dir.path().join("kaizen.db")).unwrap();
    store.upsert_session(&make_session("paged")).unwrap();
    for seq in 0..5 {
        store.append_event(&make_event("paged", seq)).unwrap();
    }
    let first = store.list_events_page("paged", 0, 2).unwrap();
    assert_eq!(first.iter().map(|e| e.seq).collect::<Vec<_>>(), vec![0, 1]);
    let second = store
        .list_events_page("paged", first[1].seq + 1, 2)
        .unwrap();
    assert_eq!(second.iter().map(|e| e.seq).collect::<Vec<_>>(), vec![2, 3]);
}

#[test]
fn append_event_dedup() {
    let dir = TempDir::new().unwrap();
    let store = Store::open(&dir.path().join("kaizen.db")).unwrap();
    store.upsert_session(&make_session("s5")).unwrap();
    store.append_event(&make_event("s5", 0)).unwrap();
    let mut duplicate = make_event("s5", 0);
    duplicate.tokens_in = Some(42);
    store.append_event(&duplicate).unwrap();
    let events = store.list_events_for_session("s5").unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].tokens_in, None);
    let aggregate = crate::extensions::aggregates::get(&store, "s5")
        .unwrap()
        .unwrap();
    assert_eq!(aggregate.event_count, 1);
}

#[test]
fn append_event_backfills_missing_aggregate_before_incrementing() {
    let dir = TempDir::new().unwrap();
    let store = Store::open(&dir.path().join("kaizen.db")).unwrap();
    store
        .upsert_session(&make_session("legacy-aggregate"))
        .unwrap();
    store
        .append_event(&make_event("legacy-aggregate", 0))
        .unwrap();
    store
        .conn()
        .execute(
            "DELETE FROM session_aggregates WHERE session_id = ?1",
            ["legacy-aggregate"],
        )
        .unwrap();

    store
        .append_event(&make_event("legacy-aggregate", 1))
        .unwrap();

    let aggregate = crate::extensions::aggregates::get(&store, "legacy-aggregate")
        .unwrap()
        .unwrap();
    assert_eq!(aggregate.event_count, 2);
}

#[test]
fn append_event_indexes_path_from_payload() {
    let dir = TempDir::new().unwrap();
    let store = Store::open(&dir.path().join("kaizen.db")).unwrap();
    store.upsert_session(&make_session("sx")).unwrap();
    let mut ev = make_event("sx", 0);
    ev.payload = json!({"input": {"path": "src/lib.rs"}});
    store.append_event(&ev).unwrap();
    let ft = store.files_touched_in_window("/ws", 0, 10_000).unwrap();
    assert_eq!(ft, vec![("sx".to_string(), "src/lib.rs".to_string())]);
}

#[test]
fn append_event_indexes_rules_from_payload() {
    let dir = TempDir::new().unwrap();
    let store = Store::open(&dir.path().join("kaizen.db")).unwrap();
    store.upsert_session(&make_session("sr")).unwrap();
    let mut ev = make_event("sr", 0);
    ev.payload = json!({"path": ".cursor/rules/my-rule.mdc"});
    store.append_event(&ev).unwrap();
    let r = store.rules_used_in_window("/ws", 0, 10_000).unwrap();
    assert_eq!(r, vec![("sr".to_string(), "my-rule".to_string())]);
}

#[test]
fn append_event_does_not_create_hot_mirror() {
    let dir = TempDir::new().unwrap();
    let store = Store::open(&dir.path().join("kaizen.db")).unwrap();
    store.upsert_session(&make_session("single-copy")).unwrap();
    store.append_event(&make_event("single-copy", 0)).unwrap();
    assert!(!dir.path().join("hot").exists());
}

#[test]
fn span_tree_cache_hits_empty_and_invalidates_on_append() {
    let dir = TempDir::new().unwrap();
    let store = Store::open(&dir.path().join("kaizen.db")).unwrap();
    assert!(store.session_span_tree("missing").unwrap().is_empty());
    assert!(store.span_tree_cache.borrow().is_some());

    store.upsert_session(&make_session("tree")).unwrap();
    let call = make_event("tree", 0);
    store.append_event(&call).unwrap();
    assert!(store.span_tree_cache.borrow().is_none());
    assert!(store.session_span_tree("tree").unwrap().is_empty());
    assert!(store.span_tree_cache.borrow().is_some());
    let mut result = make_event("tree", 1);
    result.kind = EventKind::ToolResult;
    result.tool_call_id = call.tool_call_id.clone();
    store.append_event(&result).unwrap();
    assert!(store.span_tree_cache.borrow().is_none());
    assert_eq!(store.session_span_tree("tree").unwrap().len(), 1);
}

#[test]
fn reopen_defers_projector_replay_until_next_write() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("kaizen.db");
    let mut session = make_session("lazy");
    session.status = SessionStatus::Running;
    let call = make_event("lazy", 0);
    let store = Store::open(&path).unwrap();
    store.upsert_session(&session).unwrap();
    store.append_event(&call).unwrap();
    drop(store);

    let store = Store::open(&path).unwrap();
    assert_eq!(store.projector.borrow().last_seq("lazy"), None);
    let mut result = make_event("lazy", 1);
    result.kind = EventKind::ToolResult;
    result.tool_call_id = call.tool_call_id;
    store.append_event(&result).unwrap();
    assert_eq!(store.tool_spans_for_session("lazy").unwrap().len(), 1);
}
