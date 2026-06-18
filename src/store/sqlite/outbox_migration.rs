use super::*;
use crate::store::outbox_redb::{LegacyOutbox, LegacyOutboxRow};
use anyhow::bail;
use rusqlite::Transaction;

const MARKER_KEY: &str = "outbox_redb_migration_v1_digest";
const REPLACEMENT_KINDS: [&str; 3] = ["tool_spans", "repo_snapshots", "workspace_facts"];

#[derive(Clone, Debug)]
struct QueueRow {
    owner_id: String,
    kind: String,
    payload: String,
}

pub(super) fn migrate(conn: &Connection, root: &Path) -> Result<()> {
    let source = root.join("hot").join("outbox.redb");
    if !source.try_exists()? {
        return Ok(());
    }
    let legacy = LegacyOutbox::open(&source)?;
    let rows = legacy.pending_rows()?;
    migrate_rows(conn, &rows, &digest(&rows))?;
    drop(legacy);
    rename_source(&source)
}

fn migrate_rows(conn: &Connection, legacy: &[LegacyOutboxRow], digest: &str) -> Result<()> {
    let tx = Transaction::new_unchecked(conn, TransactionBehavior::Immediate)?;
    if marker_matches(&tx, digest)? {
        tx.commit()?;
        return Ok(());
    }
    if legacy.is_empty() {
        return commit_marker(tx, digest);
    }
    let merged = reconcile(legacy, sqlite_pending(&tx)?);
    replace_pending(&tx, &merged)?;
    commit_marker(tx, digest)
}

fn commit_marker(tx: Transaction<'_>, digest: &str) -> Result<()> {
    write_marker(&tx, digest)?;
    tx.commit()?;
    Ok(())
}

fn sqlite_pending(tx: &Transaction<'_>) -> Result<Vec<QueueRow>> {
    let mut stmt = tx.prepare(
        "SELECT session_id, kind, payload FROM sync_outbox
         WHERE sent = 0 ORDER BY id",
    )?;
    let rows = stmt.query_map([], read_queue_row)?;
    rows.collect::<rusqlite::Result<_>>().map_err(Into::into)
}

fn read_queue_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<QueueRow> {
    Ok(QueueRow {
        owner_id: row.get(0)?,
        kind: row.get(1)?,
        payload: row.get(2)?,
    })
}

fn reconcile(legacy: &[LegacyOutboxRow], sqlite: Vec<QueueRow>) -> Vec<QueueRow> {
    let replacements = replacement_keys(legacy);
    let mut exact = exact_counts(legacy);
    let unmatched = sqlite
        .into_iter()
        .filter(|row| keep_sqlite(row, &replacements, &mut exact));
    legacy.iter().map(QueueRow::from).chain(unmatched).collect()
}

fn replacement_keys(rows: &[LegacyOutboxRow]) -> HashSet<(String, String)> {
    rows.iter()
        .filter(|row| is_replacement(&row.kind))
        .map(|row| (row.owner_id.clone(), row.kind.clone()))
        .collect()
}

fn exact_counts(rows: &[LegacyOutboxRow]) -> HashMap<(String, String, String), usize> {
    rows.iter()
        .filter(|row| !is_replacement(&row.kind))
        .fold(HashMap::new(), |mut counts, row| {
            *counts.entry(exact_legacy(row)).or_default() += 1;
            counts
        })
}

fn keep_sqlite(
    row: &QueueRow,
    replacements: &HashSet<(String, String)>,
    exact: &mut HashMap<(String, String, String), usize>,
) -> bool {
    if replacements.contains(&(row.owner_id.clone(), row.kind.clone())) {
        return false;
    }
    consume_exact(exact, exact_sqlite(row))
}

fn consume_exact(
    counts: &mut HashMap<(String, String, String), usize>,
    key: (String, String, String),
) -> bool {
    let Some(count) = counts.get_mut(&key) else {
        return true;
    };
    if *count == 0 {
        return true;
    }
    *count -= 1;
    false
}

fn replace_pending(tx: &Transaction<'_>, rows: &[QueueRow]) -> Result<()> {
    tx.execute("DELETE FROM sync_outbox WHERE sent = 0", [])?;
    rows.iter().try_for_each(|row| {
        tx.execute(
            "INSERT INTO sync_outbox (session_id, kind, payload, sent)
             VALUES (?1, ?2, ?3, 0)",
            params![row.owner_id, row.kind, row.payload],
        )
        .map(|_| ())
    })?;
    Ok(())
}

fn marker_matches(tx: &Transaction<'_>, digest: &str) -> Result<bool> {
    let marker = tx
        .query_row(
            "SELECT v FROM sync_state WHERE k = ?1",
            [MARKER_KEY],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    Ok(marker.as_deref() == Some(digest))
}

fn write_marker(tx: &Transaction<'_>, digest: &str) -> Result<()> {
    tx.execute(
        "INSERT INTO sync_state (k, v) VALUES (?1, ?2)
         ON CONFLICT(k) DO UPDATE SET v = excluded.v",
        params![MARKER_KEY, digest],
    )?;
    Ok(())
}

fn digest(rows: &[LegacyOutboxRow]) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"kaizen-outbox-migration-v1");
    rows.iter().for_each(|row| hash_row(&mut hasher, row));
    hasher.finalize().to_hex().to_string()
}

fn hash_row(hasher: &mut blake3::Hasher, row: &LegacyOutboxRow) {
    hasher.update(&row.id.to_le_bytes());
    hash_field(hasher, row.owner_id.as_bytes());
    hash_field(hasher, row.kind.as_bytes());
    hash_field(hasher, row.payload.as_bytes());
}

fn hash_field(hasher: &mut blake3::Hasher, value: &[u8]) {
    hasher.update(&(value.len() as u64).to_le_bytes());
    hasher.update(value);
}

fn exact_legacy(row: &LegacyOutboxRow) -> (String, String, String) {
    (row.owner_id.clone(), row.kind.clone(), row.payload.clone())
}

fn exact_sqlite(row: &QueueRow) -> (String, String, String) {
    (row.owner_id.clone(), row.kind.clone(), row.payload.clone())
}

fn is_replacement(kind: &str) -> bool {
    REPLACEMENT_KINDS.contains(&kind)
}

fn rename_source(source: &Path) -> Result<()> {
    let target = source.with_file_name("outbox.redb.migrated-v1");
    if target.try_exists()? {
        bail!(
            "legacy outbox migration target exists: {}",
            target.display()
        );
    }
    std::fs::rename(source, &target)
        .with_context(|| format!("archive legacy outbox: {}", source.display()))?;
    Ok(())
}

impl From<&LegacyOutboxRow> for QueueRow {
    fn from(row: &LegacyOutboxRow) -> Self {
        Self {
            owner_id: row.owner_id.clone(),
            kind: row.kind.clone(),
            payload: row.payload.clone(),
        }
    }
}
