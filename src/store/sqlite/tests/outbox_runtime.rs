use super::super::Store;
use super::{make_event, make_session};
use crate::core::config::SyncConfig;
use crate::sync::context::SyncIngestContext;
use tempfile::TempDir;

#[test]
fn runtime_outbox_uses_sqlite_without_creating_redb() {
    let dir = TempDir::new().unwrap();
    let store = Store::open(&dir.path().join("kaizen.db")).unwrap();
    store.upsert_session(&make_session("runtime")).unwrap();
    let ctx = SyncIngestContext::new(sync_config(), dir.path().into());
    store
        .append_event_with_sync(&make_event("runtime", 0), Some(&ctx))
        .unwrap();
    store
        .replace_outbox_rows("workspace", "workspace_facts", &["fact".into()])
        .unwrap();

    assert_eq!(store.outbox_pending_count().unwrap(), 2);
    let rows = store.list_outbox_pending(10).unwrap();
    let ids = rows.iter().map(|row| row.0).collect::<Vec<_>>();
    store.mark_outbox_sent(&ids).unwrap();
    assert_eq!(store.outbox_pending_count().unwrap(), 0);
    assert!(!dir.path().join("hot/outbox.redb").exists());
}

#[test]
fn replace_outbox_rows_rolls_back_on_insert_failure() {
    let dir = TempDir::new().unwrap();
    let store = Store::open(&dir.path().join("kaizen.db")).unwrap();
    seed(&store, "owner", "tool_spans", "old");
    fail_on_payload(&store, "boom", "replace_abort");

    let rows = ["new".to_string(), "boom".to_string()];
    assert!(
        store
            .replace_outbox_rows("owner", "tool_spans", &rows)
            .is_err()
    );
    assert_eq!(payloads(&store), vec!["old"]);
}

#[test]
fn mark_outbox_sent_rolls_back_whole_batch() {
    let dir = TempDir::new().unwrap();
    let store = Store::open(&dir.path().join("kaizen.db")).unwrap();
    seed(&store, "owner", "events", "ok");
    seed(&store, "owner", "events", "boom");
    fail_on_update(&store);
    let ids = store
        .list_outbox_pending(10)
        .unwrap()
        .into_iter()
        .map(|row| row.0)
        .collect::<Vec<_>>();

    assert!(store.mark_outbox_sent(&ids).is_err());
    assert_eq!(payloads(&store), vec!["ok", "boom"]);
}

fn sync_config() -> SyncConfig {
    SyncConfig {
        endpoint: "http://sync.invalid".into(),
        team_token: "token".into(),
        team_id: "team".into(),
        team_salt_hex: "00".repeat(32),
        ..SyncConfig::default()
    }
}

fn seed(store: &Store, owner: &str, kind: &str, payload: &str) {
    store
        .conn
        .execute(
            "INSERT INTO sync_outbox (session_id, kind, payload, sent)
             VALUES (?1, ?2, ?3, 0)",
            rusqlite::params![owner, kind, payload],
        )
        .unwrap();
}

fn fail_on_payload(store: &Store, payload: &str, trigger: &str) {
    store
        .conn
        .execute_batch(&format!(
            "CREATE TRIGGER {trigger} BEFORE INSERT ON sync_outbox
             WHEN NEW.payload = '{payload}'
             BEGIN SELECT RAISE(ABORT, 'forced failure'); END;"
        ))
        .unwrap();
}

fn fail_on_update(store: &Store) {
    store
        .conn
        .execute_batch(
            "CREATE TRIGGER ack_abort BEFORE UPDATE OF sent ON sync_outbox
             WHEN OLD.payload = 'boom'
             BEGIN SELECT RAISE(ABORT, 'forced failure'); END;",
        )
        .unwrap();
}

fn payloads(store: &Store) -> Vec<String> {
    store
        .list_outbox_pending(10)
        .unwrap()
        .into_iter()
        .map(|row| row.2)
        .collect()
}
