use super::super::{SessionFilter, Store};
use super::{SessionStatus, make_session};
use tempfile::TempDir;

#[test]
fn upsert_and_get_session() {
    let dir = TempDir::new().unwrap();
    let store = Store::open(&dir.path().join("kaizen.db")).unwrap();
    let s = make_session("s1");
    store.upsert_session(&s).unwrap();
    let got = store.get_session("s1").unwrap().unwrap();
    assert_eq!(got.id, "s1");
    assert_eq!(got.status, SessionStatus::Done);
}

#[test]
fn list_sessions_page_orders_and_counts() {
    let dir = TempDir::new().unwrap();
    let store = Store::open(&dir.path().join("kaizen.db")).unwrap();
    let mut a = make_session("a");
    a.started_at_ms = 2_000;
    let mut b = make_session("b");
    b.started_at_ms = 2_000;
    let mut c = make_session("c");
    c.started_at_ms = 1_000;
    store.upsert_session(&c).unwrap();
    store.upsert_session(&b).unwrap();
    store.upsert_session(&a).unwrap();

    let page = store
        .list_sessions_page("/ws", 0, 2, SessionFilter::default())
        .unwrap();
    assert_eq!(page.total, 3);
    assert_eq!(page.next_offset, Some(2));
    assert_eq!(
        page.rows.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
        vec!["a", "b"]
    );
    let all = store.list_sessions("/ws").unwrap();
    assert_eq!(
        all.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
        vec!["a", "b", "c"]
    );
}

#[test]
fn list_sessions_page_filters_in_sql_shape() {
    let dir = TempDir::new().unwrap();
    let store = Store::open(&dir.path().join("kaizen.db")).unwrap();
    let mut cursor = make_session("cursor");
    cursor.agent = "Cursor".into();
    cursor.started_at_ms = 2_000;
    cursor.status = SessionStatus::Running;
    let mut claude = make_session("claude");
    claude.agent = "claude".into();
    claude.started_at_ms = 3_000;
    store.upsert_session(&cursor).unwrap();
    store.upsert_session(&claude).unwrap();

    let page = store
        .list_sessions_page(
            "/ws",
            0,
            10,
            SessionFilter {
                agent_prefix: Some("cur".into()),
                status: Some(SessionStatus::Running),
                since_ms: Some(1_500),
            },
        )
        .unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.rows[0].id, "cursor");
}

#[test]
fn incremental_session_helpers_find_new_rows_and_statuses() {
    let dir = TempDir::new().unwrap();
    let store = Store::open(&dir.path().join("kaizen.db")).unwrap();
    let mut old = make_session("old");
    old.started_at_ms = 1_000;
    let mut new = make_session("new");
    new.started_at_ms = 2_000;
    new.status = SessionStatus::Running;
    store.upsert_session(&old).unwrap();
    store.upsert_session(&new).unwrap();

    let rows = store.list_sessions_started_after("/ws", 1_500).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, "new");
    store
        .update_session_status("new", SessionStatus::Done)
        .unwrap();
    let statuses = store.session_statuses(&["new".to_string()]).unwrap();
    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].status, SessionStatus::Done);
}

#[test]
fn upsert_idempotent() {
    let dir = TempDir::new().unwrap();
    let store = Store::open(&dir.path().join("kaizen.db")).unwrap();
    let mut s = make_session("s3");
    store.upsert_session(&s).unwrap();
    s.status = SessionStatus::Running;
    store.upsert_session(&s).unwrap();
    let got = store.get_session("s3").unwrap().unwrap();
    assert_eq!(got.status, SessionStatus::Running);
}

#[test]
fn update_session_status_changes_status() {
    let dir = TempDir::new().unwrap();
    let store = Store::open(&dir.path().join("kaizen.db")).unwrap();
    store.upsert_session(&make_session("s6")).unwrap();
    store
        .update_session_status("s6", SessionStatus::Running)
        .unwrap();
    let got = store.get_session("s6").unwrap().unwrap();
    assert_eq!(got.status, SessionStatus::Running);
}
