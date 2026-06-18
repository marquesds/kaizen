use super::super::Store;
use super::outbox_support::{
    create_legacy, legacy_path, migrated_path, migration_marker, pending_rows, seed_sqlite,
};
use tempfile::TempDir;

#[test]
fn migration_orders_legacy_before_reconciled_sqlite_rows() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("kaizen.db");
    let store = Store::open(&db).unwrap();
    seed_sqlite(
        &store,
        &[
            ("event-owner", "events", "duplicate"),
            ("sqlite-only", "events", "sqlite-event"),
            ("replace", "tool_spans", "stale-span"),
            ("snapshot", "repo_snapshots", "stale-snapshot"),
            ("workspace", "workspace_facts", "stale-facts"),
            ("sqlite-replace", "tool_spans", "sqlite-span"),
            ("unknown", "custom_kind", "duplicate-custom"),
            ("event-owner", "events", "duplicate"),
            ("event-owner", "events", "duplicate"),
        ],
    );
    drop(store);
    drop(
        create_legacy(
            dir.path(),
            &[
                ("event-owner", "events", "duplicate"),
                ("replace", "tool_spans", "legacy-span-a"),
                ("snapshot", "repo_snapshots", "legacy-snapshot"),
                ("unknown", "custom_kind", "duplicate-custom"),
                ("workspace", "workspace_facts", "legacy-facts"),
                ("event-owner", "events", "duplicate"),
                ("replace", "tool_spans", "legacy-span-b"),
            ],
        )
        .unwrap(),
    );

    let store = Store::open(&db).unwrap();
    assert_eq!(
        pending_rows(&store),
        vec![
            row("event-owner", "events", "duplicate"),
            row("replace", "tool_spans", "legacy-span-a"),
            row("snapshot", "repo_snapshots", "legacy-snapshot"),
            row("unknown", "custom_kind", "duplicate-custom"),
            row("workspace", "workspace_facts", "legacy-facts"),
            row("event-owner", "events", "duplicate"),
            row("replace", "tool_spans", "legacy-span-b"),
            row("sqlite-only", "events", "sqlite-event"),
            row("sqlite-replace", "tool_spans", "sqlite-span"),
            row("event-owner", "events", "duplicate"),
        ]
    );
    assert!(migration_marker(&store).is_some());
    assert!(!legacy_path(dir.path()).exists());
    assert!(migrated_path(dir.path()).exists());
}

#[test]
fn committed_migration_retries_rename_without_duplicating_rows() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("kaizen.db");
    let store = Store::open(&db).unwrap();
    seed_sqlite(&store, &[("sqlite", "events", "sqlite-row")]);
    drop(store);
    drop(create_legacy(dir.path(), &[("legacy", "events", "legacy-row")]).unwrap());
    std::fs::create_dir(migrated_path(dir.path())).unwrap();

    assert!(Store::open(&db).is_err());
    let store = Store::open_read_only(&db).unwrap();
    assert_eq!(
        pending_rows(&store),
        vec![
            row("legacy", "events", "legacy-row"),
            row("sqlite", "events", "sqlite-row"),
        ]
    );
    assert!(migration_marker(&store).is_some());
    drop(store);

    std::fs::remove_dir(migrated_path(dir.path())).unwrap();
    let store = Store::open(&db).unwrap();
    assert_eq!(
        pending_rows(&store),
        vec![
            row("legacy", "events", "legacy-row"),
            row("sqlite", "events", "sqlite-row"),
        ]
    );
    assert!(migrated_path(dir.path()).is_file());
}

#[test]
fn empty_legacy_outbox_preserves_sqlite_row_ids() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("kaizen.db");
    let store = Store::open(&db).unwrap();
    seed_sqlite(&store, &[("sqlite", "events", "sqlite-row")]);
    let id = store.list_outbox_pending(1).unwrap()[0].0;
    drop(store);
    drop(create_legacy(dir.path(), &[]).unwrap());

    let store = Store::open(&db).unwrap();
    assert_eq!(store.list_outbox_pending(1).unwrap()[0].0, id);
    assert!(migration_marker(&store).is_some());
    assert!(migrated_path(dir.path()).is_file());
}

fn row(owner: &str, kind: &str, payload: &str) -> (String, String, String) {
    (owner.into(), kind.into(), payload.into())
}
