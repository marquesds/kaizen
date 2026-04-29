// SPDX-License-Identifier: AGPL-3.0-or-later
//! redb sync outbox: append + drain queue, one writer, many readers.

use anyhow::Result;
use redb::{Database, ReadableDatabase, ReadableTable, ReadableTableMetadata, TableDefinition};
use std::path::Path;

const ROWS: TableDefinition<u64, &[u8]> = TableDefinition::new("rows");
const META: TableDefinition<&str, u64> = TableDefinition::new("meta");

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct Row {
    owner_id: String,
    kind: String,
    payload: String,
}

pub struct Outbox {
    db: Database,
}

impl Outbox {
    pub fn open(root: &Path) -> Result<Self> {
        let dir = root.join("hot");
        std::fs::create_dir_all(&dir)?;
        let db = Database::create(dir.join("outbox.redb"))?;
        let tx = db.begin_write()?;
        tx.open_table(ROWS)?;
        tx.open_table(META)?;
        tx.commit()?;
        Ok(Self { db })
    }

    pub fn append(&self, owner_id: &str, kind: &str, payload: &str) -> Result<u64> {
        let tx = self.db.begin_write()?;
        let id = next_id(&tx)?;
        let row = serde_json::to_vec(&Row {
            owner_id: owner_id.into(),
            kind: kind.into(),
            payload: payload.into(),
        })?;
        tx.open_table(ROWS)?.insert(id, row.as_slice())?;
        tx.commit()?;
        Ok(id)
    }

    pub fn list_pending(&self, limit: usize) -> Result<Vec<(i64, String, String)>> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(ROWS)?;
        let mut out = Vec::new();
        for row in table.iter()? {
            let (id, bytes) = row?;
            let q: Row = serde_json::from_slice(bytes.value())?;
            out.push((id.value() as i64, q.kind, q.payload));
            if out.len() >= limit {
                break;
            }
        }
        Ok(out)
    }

    pub fn delete_ids(&self, ids: &[i64]) -> Result<()> {
        let tx = self.db.begin_write()?;
        {
            let mut table = tx.open_table(ROWS)?;
            for id in ids {
                table.remove(*id as u64)?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn replace(&self, owner_id: &str, kind: &str, payloads: &[String]) -> Result<()> {
        let tx = self.db.begin_write()?;
        let mut delete = Vec::new();
        {
            let table = tx.open_table(ROWS)?;
            for row in table.iter()? {
                let (id, bytes) = row?;
                let q: Row = serde_json::from_slice(bytes.value())?;
                if q.owner_id == owner_id && q.kind == kind {
                    delete.push(id.value());
                }
            }
        }
        let mut next = next_id_value(&tx)?;
        {
            let mut table = tx.open_table(ROWS)?;
            for id in delete {
                table.remove(id)?;
            }
            for payload in payloads {
                let row = serde_json::to_vec(&Row {
                    owner_id: owner_id.into(),
                    kind: kind.into(),
                    payload: payload.clone(),
                })?;
                table.insert(next, row.as_slice())?;
                next += 1;
            }
        }
        tx.open_table(META)?.insert("next_id", next)?;
        tx.commit()?;
        Ok(())
    }

    pub fn pending_count(&self) -> Result<u64> {
        Ok(self.db.begin_read()?.open_table(ROWS)?.len()?)
    }
}

fn next_id(tx: &redb::WriteTransaction) -> Result<u64> {
    let id = next_id_value(tx)?;
    tx.open_table(META)?.insert("next_id", id + 1)?;
    Ok(id)
}

fn next_id_value(tx: &redb::WriteTransaction) -> Result<u64> {
    Ok(tx
        .open_table(META)?
        .get("next_id")?
        .map(|v| v.value())
        .unwrap_or(1))
}
