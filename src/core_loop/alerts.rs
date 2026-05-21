// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::core_loop::{AlertEvent, AlertSeverity};
use crate::store::Store;
use anyhow::Result;
use rusqlite::{OptionalExtension, params};

pub use crate::core_loop::alert_checks::check_builtin;

pub fn emit(
    store: &Store,
    source_key: &str,
    name: &str,
    severity: AlertSeverity,
    message: &str,
    session_id: Option<&str>,
    now_ms: u64,
) -> Result<AlertEvent> {
    let id = uuid::Uuid::now_v7().to_string();
    store.conn().execute(
        "INSERT OR IGNORE INTO alert_events
         (id, source_key, name, severity, message, session_id, created_at_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            id,
            source_key,
            name,
            severity.as_str(),
            message,
            session_id,
            now_ms as i64
        ],
    )?;
    by_source(store, source_key)
}

pub fn list(store: &Store) -> Result<Vec<AlertEvent>> {
    let mut stmt = store.conn().prepare("SELECT id, source_key, name, severity, message, session_id, created_at_ms FROM alert_events ORDER BY created_at_ms DESC")?;
    let rows = stmt.query_map([], row)?;
    rows.map(|r| r.map_err(anyhow::Error::from)).collect()
}

fn by_source(store: &Store, source_key: &str) -> Result<AlertEvent> {
    let sql = "SELECT id, source_key, name, severity, message, session_id, created_at_ms FROM alert_events WHERE source_key = ?1";
    store
        .conn()
        .query_row(sql, params![source_key], row)
        .optional()?
        .ok_or_else(|| anyhow::anyhow!("alert missing after insert"))
}

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<AlertEvent> {
    Ok(AlertEvent {
        id: r.get(0)?,
        source_key: r.get(1)?,
        name: r.get(2)?,
        severity: severity(r.get::<_, String>(3)?.as_str()),
        message: r.get(4)?,
        session_id: r.get(5)?,
        created_at_ms: r.get::<_, i64>(6)? as u64,
    })
}

fn severity(s: &str) -> AlertSeverity {
    match s {
        "critical" => AlertSeverity::Critical,
        "info" => AlertSeverity::Info,
        _ => AlertSeverity::Warning,
    }
}
