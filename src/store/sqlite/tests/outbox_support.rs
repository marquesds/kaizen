use super::super::Store;
use anyhow::Result;
use redb::{Database, TableDefinition};
use rusqlite::OptionalExtension;
use serde::Serialize;
use std::path::{Path, PathBuf};

const ROWS: TableDefinition<u64, &[u8]> = TableDefinition::new("rows");
const META: TableDefinition<&str, u64> = TableDefinition::new("meta");
pub(super) type RowSpec<'a> = (&'a str, &'a str, &'a str);

#[derive(Serialize)]
struct LegacyRow<'a> {
    owner_id: &'a str,
    kind: &'a str,
    payload: &'a str,
}

pub(super) fn create_legacy(root: &Path, rows: &[RowSpec<'_>]) -> Result<Database> {
    std::fs::create_dir_all(root.join("hot"))?;
    let db = Database::create(legacy_path(root))?;
    let tx = db.begin_write()?;
    write_legacy_rows(&tx, rows)?;
    tx.commit()?;
    Ok(db)
}

fn write_legacy_rows(tx: &redb::WriteTransaction, rows: &[RowSpec<'_>]) -> Result<()> {
    let mut table = tx.open_table(ROWS)?;
    for (index, (owner_id, kind, payload)) in rows.iter().enumerate() {
        let row = LegacyRow {
            owner_id,
            kind,
            payload,
        };
        table.insert(index as u64 + 1, serde_json::to_vec(&row)?.as_slice())?;
    }
    drop(table);
    tx.open_table(META)?
        .insert("next_id", rows.len() as u64 + 1)?;
    Ok(())
}

pub(super) fn seed_sqlite(store: &Store, rows: &[RowSpec<'_>]) {
    rows.iter().for_each(|(owner, kind, payload)| {
        store
            .conn
            .execute(
                "INSERT INTO sync_outbox (session_id, kind, payload, sent)
                 VALUES (?1, ?2, ?3, 0)",
                rusqlite::params![owner, kind, payload],
            )
            .unwrap();
    });
}

pub(super) fn pending_rows(store: &Store) -> Vec<(String, String, String)> {
    let mut stmt = store
        .conn
        .prepare(
            "SELECT session_id, kind, payload FROM sync_outbox
             WHERE sent = 0 ORDER BY id",
        )
        .unwrap();
    stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

pub(super) fn migration_marker(store: &Store) -> Option<String> {
    store
        .conn
        .query_row(
            "SELECT v FROM sync_state WHERE k = 'outbox_redb_migration_v1_digest'",
            [],
            |row| row.get(0),
        )
        .optional()
        .unwrap()
}

pub(super) fn legacy_path(root: &Path) -> PathBuf {
    root.join("hot").join("outbox.redb")
}

pub(super) fn migrated_path(root: &Path) -> PathBuf {
    root.join("hot").join("outbox.redb.migrated-v1")
}
