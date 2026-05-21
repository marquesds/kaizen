// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::core_loop::{ReviewItem, ReviewStatus};
use crate::store::Store;
use anyhow::{Result, anyhow};
use rusqlite::{OptionalExtension, params};

pub fn create(
    store: &Store,
    source_key: &str,
    session_id: &str,
    title: &str,
    now_ms: u64,
) -> Result<ReviewItem> {
    let id = uuid::Uuid::now_v7().to_string();
    store.conn().execute(
        "INSERT OR IGNORE INTO review_items
         (id, source_key, session_id, title, status, created_at_ms, resolved_at_ms)
         VALUES (?1, ?2, ?3, ?4, 'open', ?5, NULL)",
        params![id, source_key, session_id, title, now_ms as i64],
    )?;
    by_source(store, source_key)
}

pub fn list(store: &Store, status: Option<ReviewStatus>) -> Result<Vec<ReviewItem>> {
    let sql = "SELECT id, source_key, session_id, title, status, created_at_ms, resolved_at_ms FROM review_items WHERE (?1 IS NULL OR status = ?1) ORDER BY created_at_ms DESC";
    let mut stmt = store.conn().prepare(sql)?;
    let rows = stmt.query_map(params![status.map(|s| s.as_str().to_string())], row)?;
    rows.map(|r| r.map_err(anyhow::Error::from)).collect()
}

pub fn get(store: &Store, id: &str) -> Result<ReviewItem> {
    let sql = "SELECT id, source_key, session_id, title, status, created_at_ms, resolved_at_ms FROM review_items WHERE id = ?1";
    store
        .conn()
        .query_row(sql, params![id], row)
        .optional()?
        .ok_or_else(|| anyhow!("review not found: {id}"))
}

pub fn set_status(store: &Store, id: &str, status: ReviewStatus, now_ms: u64) -> Result<()> {
    store.conn().execute(
        "UPDATE review_items SET status = ?2, resolved_at_ms = ?3 WHERE id = ?1",
        params![id, status.as_str(), now_ms as i64],
    )?;
    Ok(())
}

fn by_source(store: &Store, source_key: &str) -> Result<ReviewItem> {
    let sql = "SELECT id, source_key, session_id, title, status, created_at_ms, resolved_at_ms FROM review_items WHERE source_key = ?1";
    store
        .conn()
        .query_row(sql, params![source_key], row)
        .map_err(Into::into)
}

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<ReviewItem> {
    Ok(ReviewItem {
        id: r.get(0)?,
        source_key: r.get(1)?,
        session_id: r.get(2)?,
        title: r.get(3)?,
        status: status(r.get::<_, String>(4)?.as_str()),
        created_at_ms: r.get::<_, i64>(5)? as u64,
        resolved_at_ms: r.get::<_, Option<i64>>(6)?.map(|v| v as u64),
    })
}

fn status(s: &str) -> ReviewStatus {
    match s {
        "resolved" => ReviewStatus::Resolved,
        "dismissed" => ReviewStatus::Dismissed,
        _ => ReviewStatus::Open,
    }
}
