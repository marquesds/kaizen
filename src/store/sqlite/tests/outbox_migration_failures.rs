use super::super::Store;
use super::outbox_support::{
    create_legacy, legacy_path, migrated_path, migration_marker, pending_rows, seed_sqlite,
};
use tempfile::TempDir;

#[test]
fn locked_legacy_outbox_fails_without_mutating_either_store() {
    let dir = TempDir::new().unwrap();
    let db = seeded_db(&dir);
    let legacy = create_legacy(dir.path(), &[("legacy", "events", "row")]).unwrap();

    assert!(Store::open(&db).is_err());
    assert_untouched(&dir, &db);
    drop(legacy);
}

#[test]
fn corrupt_legacy_outbox_fails_without_mutating_either_store() {
    let dir = TempDir::new().unwrap();
    let db = seeded_db(&dir);
    std::fs::create_dir_all(dir.path().join("hot")).unwrap();
    std::fs::write(legacy_path(dir.path()), b"not a redb database").unwrap();
    let before = std::fs::read(legacy_path(dir.path())).unwrap();

    assert!(Store::open(&db).is_err());
    assert_untouched(&dir, &db);
    assert_eq!(std::fs::read(legacy_path(dir.path())).unwrap(), before);
}

#[test]
fn read_only_open_never_migrates_legacy_outbox() {
    let dir = TempDir::new().unwrap();
    let db = seeded_db(&dir);
    drop(create_legacy(dir.path(), &[("legacy", "events", "row")]).unwrap());

    let store = Store::open_read_only(&db).unwrap();
    assert_eq!(pending_rows(&store), vec![sqlite_row()]);
    assert!(legacy_path(dir.path()).is_file());
    assert!(!migrated_path(dir.path()).exists());
}

fn seeded_db(dir: &TempDir) -> std::path::PathBuf {
    let db = dir.path().join("kaizen.db");
    let store = Store::open(&db).unwrap();
    seed_sqlite(&store, &[("sqlite", "events", "sqlite-row")]);
    db
}

fn assert_untouched(dir: &TempDir, db: &std::path::Path) {
    let store = Store::open_read_only(db).unwrap();
    assert_eq!(pending_rows(&store), vec![sqlite_row()]);
    assert!(migration_marker(&store).is_none());
    assert!(legacy_path(dir.path()).is_file());
    assert!(!migrated_path(dir.path()).exists());
}

fn sqlite_row() -> (String, String, String) {
    ("sqlite".into(), "events".into(), "sqlite-row".into())
}
