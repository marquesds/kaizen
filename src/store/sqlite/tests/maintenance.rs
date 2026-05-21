use super::super::{PruneStats, Store};
use super::{make_event, make_session};
use serde_json::json;
use tempfile::TempDir;

#[test]
fn prune_sessions_removes_old_rows_and_keeps_recent() {
    let dir = TempDir::new().unwrap();
    let store = Store::open(&dir.path().join("kaizen.db")).unwrap();
    let mut old = make_session("old");
    old.started_at_ms = 1_000;
    let mut new = make_session("new");
    new.started_at_ms = 9_000_000_000_000;
    store.upsert_session(&old).unwrap();
    store.upsert_session(&new).unwrap();
    store.append_event(&make_event("old", 0)).unwrap();

    let stats = store.prune_sessions_started_before(5_000).unwrap();
    assert_eq!(
        stats,
        PruneStats {
            sessions_removed: 1,
            events_removed: 1,
        }
    );
    assert!(store.get_session("old").unwrap().is_none());
    assert!(store.get_session("new").unwrap().is_some());
    let sessions = store.list_sessions("/ws").unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id, "new");
}

#[test]
fn prune_sessions_removes_rules_used_rows() {
    let dir = TempDir::new().unwrap();
    let store = Store::open(&dir.path().join("kaizen.db")).unwrap();
    let mut old = make_session("old_r");
    old.started_at_ms = 1_000;
    store.upsert_session(&old).unwrap();
    let mut ev = make_event("old_r", 0);
    ev.payload = json!({"path": ".cursor/rules/x.mdc"});
    store.append_event(&ev).unwrap();

    store.prune_sessions_started_before(5_000).unwrap();
    let n: i64 = store
        .conn
        .query_row("SELECT COUNT(*) FROM rules_used", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 0);
}
