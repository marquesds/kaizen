use super::super::Store;
use super::{make_event, make_session};
use crate::core::event::EventKind;
use rusqlite::params;
use tempfile::TempDir;

const COLLISION_COUNTS_SQL: &str = "
    SELECT COUNT(*), COUNT(DISTINCT span_id),
           COUNT(CASE WHEN session_id = 'collision-a' AND tool_call_id = 'shared-call' THEN 1 END),
           COUNT(CASE WHEN session_id = 'collision-b' AND tool_call_id = 'shared-call' THEN 1 END)
    FROM tool_spans";

#[test]
fn identical_raw_tool_call_ids_persist_distinct_session_attribution() {
    let dir = TempDir::new().unwrap();
    let store = Store::open(&dir.path().join("kaizen.db")).unwrap();
    persist_completed_span(&store, "collision-a");
    persist_completed_span(&store, "collision-b");
    assert_eq!(collision_counts(&store), (2, 2, 1, 1));
}

fn persist_completed_span(store: &Store, session_id: &str) {
    store.upsert_session(&make_session(session_id)).unwrap();
    let mut call = make_event(session_id, 0);
    call.tool_call_id = Some("shared-call".to_string());
    store.append_event(&call).unwrap();
    let mut result = make_event(session_id, 1);
    result.kind = EventKind::ToolResult;
    result.tool_call_id = call.tool_call_id;
    store.append_event(&result).unwrap();
}

fn collision_counts(store: &Store) -> (i64, i64, i64, i64) {
    store
        .conn
        .query_row(COLLISION_COUNTS_SQL, [], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .unwrap()
}

#[test]
fn tool_spans_in_window_uses_started_then_ended_fallback() {
    let dir = TempDir::new().unwrap();
    let store = Store::open(&dir.path().join("kaizen.db")).unwrap();
    store.upsert_session(&make_session("spans")).unwrap();
    for (id, started, ended) in [
        ("started", Some(200_i64), None),
        ("fallback", None, Some(250_i64)),
        ("outside", Some(400_i64), None),
        ("too_old", None, Some(50_i64)),
        ("started_wins", Some(500_i64), Some(200_i64)),
    ] {
        store
            .conn
            .execute(
                "INSERT INTO tool_spans
                     (span_id, session_id, tool, status, started_at_ms, ended_at_ms, paths_json)
                     VALUES (?1, 'spans', 'read', 'done', ?2, ?3, '[]')",
                params![id, started, ended],
            )
            .unwrap();
    }
    let rows = store.tool_spans_in_window("/ws", 100, 300).unwrap();
    let ids = rows.into_iter().map(|r| r.span_id).collect::<Vec<_>>();
    assert_eq!(ids, vec!["fallback".to_string(), "started".to_string()]);
}

#[test]
fn tool_spans_sync_rows_in_window_returns_session_id_with_filtering() {
    let dir = TempDir::new().unwrap();
    let store = Store::open(&dir.path().join("kaizen.db")).unwrap();
    store.upsert_session(&make_session("s1")).unwrap();
    for (id, started, ended) in [
        ("inside_started", Some(150_i64), None),
        ("inside_ended_only", None, Some(220_i64)),
        ("after_window", Some(400_i64), None),
        ("before_window", None, Some(50_i64)),
    ] {
        store
            .conn
            .execute(
                "INSERT INTO tool_spans
                     (span_id, session_id, tool, status, started_at_ms, ended_at_ms, paths_json)
                     VALUES (?1, 's1', 'read', 'done', ?2, ?3, '[]')",
                params![id, started, ended],
            )
            .unwrap();
    }
    let rows = store
        .tool_spans_sync_rows_in_window("/ws", 100, 300)
        .unwrap();
    let ids: Vec<_> = rows.iter().map(|r| r.span_id.as_str()).collect();
    assert_eq!(ids, vec!["inside_started", "inside_ended_only"]);
    assert!(rows.iter().all(|r| r.session_id == "s1"));
}

#[test]
fn limited_session_span_tree_keeps_grandchildren() {
    let store = temp_store();
    seed_span(&store, "tree", "root", None, 0, 100);
    seed_span(&store, "tree", "child", Some("root"), 1, 200);
    seed_span(&store, "tree", "grandchild", Some("child"), 2, 300);
    let roots = store.limited_session_span_tree("tree", 200).unwrap();
    assert_eq!(tree_ids(&roots), vec!["root", "child", "grandchild"]);
}

#[test]
fn limited_session_helpers_cap_at_200_rows() {
    let store = temp_store();
    (0..205).for_each(|i| seed_span(&store, "limited", &format!("s{i:03}"), None, 0, i));
    let rows = store
        .tool_spans_for_session_limited("limited", 200)
        .unwrap();
    let roots = store.limited_session_span_tree("limited", 200).unwrap();
    assert_eq!((rows.len(), node_count(&roots)), (200, 200));
}

fn temp_store() -> Store {
    let dir = TempDir::new().unwrap().keep();
    let store = Store::open(&dir.join("kaizen.db")).unwrap();
    ["tree", "limited"]
        .iter()
        .for_each(|id| store.upsert_session(&make_session(id)).unwrap());
    store
}

fn seed_span(
    store: &Store,
    session: &str,
    id: &str,
    parent: Option<&str>,
    depth: u32,
    started: u64,
) {
    store.conn.execute(
        "INSERT INTO tool_spans (span_id, session_id, tool, status, started_at_ms, paths_json, parent_span_id, depth)
         VALUES (?1, ?2, 'read', 'done', ?3, '[]', ?4, ?5)",
        params![id, session, started as i64, parent, depth as i64],
    ).unwrap();
}

fn tree_ids(nodes: &[crate::store::span_tree::SpanNode]) -> Vec<&str> {
    nodes
        .iter()
        .flat_map(|n| std::iter::once(n.span.span_id.as_str()).chain(tree_ids(&n.children)))
        .collect()
}

fn node_count(nodes: &[crate::store::span_tree::SpanNode]) -> usize {
    nodes
        .iter()
        .map(|node| 1 + node_count(&node.children))
        .sum()
}
