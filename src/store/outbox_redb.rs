// SPDX-License-Identifier: AGPL-3.0-or-later
//! Read-only adapter for migrating legacy redb sync outboxes.

use anyhow::{Context, Result};
use redb::{ReadOnlyDatabase, ReadableDatabase, ReadableTable, TableDefinition};
use std::path::Path;

const ROWS: TableDefinition<u64, &[u8]> = TableDefinition::new("rows");

#[derive(Debug, Clone)]
pub(crate) struct LegacyOutboxRow {
    pub id: u64,
    pub owner_id: String,
    pub kind: String,
    pub payload: String,
}

#[derive(serde::Deserialize)]
struct StoredRow {
    owner_id: String,
    kind: String,
    payload: String,
}

pub(crate) struct LegacyOutbox {
    db: ReadOnlyDatabase,
}

impl LegacyOutbox {
    pub fn open(path: &Path) -> Result<Self> {
        let db = ReadOnlyDatabase::open(path)
            .with_context(|| format!("open legacy outbox: {}", path.display()))?;
        Ok(Self { db })
    }

    pub fn pending_rows(&self) -> Result<Vec<LegacyOutboxRow>> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(ROWS)?;
        let mut rows = Vec::new();
        for entry in table.iter()? {
            rows.push(decode_row(entry?)?);
        }
        Ok(rows)
    }
}

fn decode_row(
    entry: (redb::AccessGuard<'_, u64>, redb::AccessGuard<'_, &[u8]>),
) -> Result<LegacyOutboxRow> {
    let (id, bytes) = entry;
    let row: StoredRow = serde_json::from_slice(bytes.value())?;
    Ok(LegacyOutboxRow {
        id: id.value(),
        owner_id: row.owner_id,
        kind: row.kind,
        payload: row.payload,
    })
}
