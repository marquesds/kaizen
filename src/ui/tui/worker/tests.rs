use super::*;
use rusqlite::params;

#[test]
fn detail_materializes_at_most_two_hundred_spans() {
    let store = test_store();
    (0..205).for_each(|index| insert_span(&store, index));
    let detail = load_detail(&store, "bounded").unwrap();
    assert_eq!(detail.tool_lead_by_call.len(), DETAIL_SPAN_LIMIT);
    assert_eq!(node_count(&detail.span_nodes), DETAIL_SPAN_LIMIT);
}

fn test_store() -> Store {
    let root = tempfile::tempdir().unwrap().keep();
    let store = Store::open(&root.join("kaizen.db")).unwrap();
    store
        .conn()
        .execute(
            "INSERT INTO sessions (id, agent, workspace, started_at_ms, status, trace_path)
         VALUES ('bounded', 'codex', '/workspace', 0, 'Running', '')",
            [],
        )
        .unwrap();
    store
}

fn insert_span(store: &Store, index: u64) {
    store
        .conn()
        .execute(
            "INSERT INTO tool_spans
         (span_id, session_id, tool_call_id, tool, status, started_at_ms, lead_time_ms, paths_json)
         VALUES (?1, 'bounded', ?2, 'read', 'done', ?3, ?3, '[]')",
            params![
                format!("span-{index:03}"),
                format!("call-{index:03}"),
                index as i64
            ],
        )
        .unwrap();
}

fn node_count(nodes: &[crate::store::SpanNode]) -> usize {
    nodes
        .iter()
        .map(|node| 1 + node_count(&node.children))
        .sum()
}
