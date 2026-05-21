use super::super::Store;
use super::{EventKind, make_event, make_session};
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
    store.append_event(&make_event("s5", 0)).unwrap();
    let events = store.list_events_for_session("s5").unwrap();
    assert_eq!(events.len(), 1);
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
