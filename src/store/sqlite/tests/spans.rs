use super::super::Store;
use super::make_session;
use rusqlite::params;
use tempfile::TempDir;

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
