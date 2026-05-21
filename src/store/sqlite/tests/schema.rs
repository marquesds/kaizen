use super::super::Store;
use super::super::schema::{column_was_added_by_race, mmap_size_bytes_from_mb};
use rusqlite::{Connection, params};
use tempfile::TempDir;

#[test]
fn open_and_wal_mode() {
    let dir = TempDir::new().unwrap();
    let store = Store::open(&dir.path().join("kaizen.db")).unwrap();
    let mode: String = store
        .conn
        .query_row("PRAGMA journal_mode", [], |r| r.get(0))
        .unwrap();
    assert_eq!(mode, "wal");
}

#[test]
fn open_applies_phase0_pragmas() {
    let dir = TempDir::new().unwrap();
    let store = Store::open(&dir.path().join("kaizen.db")).unwrap();
    let synchronous: i64 = store
        .conn
        .query_row("PRAGMA synchronous", [], |r| r.get(0))
        .unwrap();
    let cache_size: i64 = store
        .conn
        .query_row("PRAGMA cache_size", [], |r| r.get(0))
        .unwrap();
    let temp_store: i64 = store
        .conn
        .query_row("PRAGMA temp_store", [], |r| r.get(0))
        .unwrap();
    let wal_autocheckpoint: i64 = store
        .conn
        .query_row("PRAGMA wal_autocheckpoint", [], |r| r.get(0))
        .unwrap();
    assert_eq!(synchronous, 1);
    assert_eq!(cache_size, -65_536);
    assert_eq!(temp_store, 2);
    assert_eq!(wal_autocheckpoint, 1_000);
    assert_eq!(mmap_size_bytes_from_mb(Some("64")), 67_108_864);
}

#[test]
fn ensure_column_tolerates_duplicate_from_race() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE sessions (id TEXT, start_commit TEXT)")
        .unwrap();
    let err = conn
        .execute("ALTER TABLE sessions ADD COLUMN start_commit TEXT", [])
        .unwrap_err();
    assert!(column_was_added_by_race(&conn, "sessions", "start_commit", &err).unwrap());
}

#[test]
fn read_only_open_sets_query_only() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("kaizen.db");
    Store::open(&db).unwrap();
    let store = Store::open_read_only(&db).unwrap();
    let query_only: i64 = store
        .conn
        .query_row("PRAGMA query_only", [], |r| r.get(0))
        .unwrap();
    assert_eq!(query_only, 1);
}

#[test]
fn phase0_indexes_exist() {
    let dir = TempDir::new().unwrap();
    let store = Store::open(&dir.path().join("kaizen.db")).unwrap();
    for name in [
        "tool_spans_session_idx",
        "tool_spans_started_idx",
        "session_samples_ts_idx",
        "events_ts_idx",
        "feedback_session_idx",
    ] {
        let found: i64 = store
            .conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name=?1",
                params![name],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(found, 1, "{name}");
    }
}
