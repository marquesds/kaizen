// SPDX-License-Identifier: AGPL-3.0-or-later
//! Read-only Cursor/VS Code `state.vscdb` helpers.

use super::cursor_state_db_fields::{
    file_mtime_ms, id_field, key_suffix, text, ts_any, ts_field, workspace_field, workspace_matches,
};
use crate::core::event::{Event, EventKind, EventSource, SessionRecord, SessionStatus};
use anyhow::{Context, Result};
use rusqlite::types::ValueRef;
use rusqlite::{Connection, OpenFlags};
use serde_json::{Value, json};
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CursorStateItem {
    pub key: String,
    pub value: String,
}

pub fn read_items_with_prefix(db_path: &Path, prefix: &str) -> Result<Vec<CursorStateItem>> {
    let conn = open_read_only(db_path)?;
    if !has_item_table(&conn)? {
        return Ok(Vec::new());
    }
    let like = format!("{}%", escape_like(prefix));
    let mut stmt = conn
        .prepare("SELECT key, value FROM ItemTable WHERE key LIKE ?1 ESCAPE '\\' ORDER BY key")?;
    let rows = stmt.query_map([like], row_item)?;
    rows.map(|r| r.map_err(anyhow::Error::from)).collect()
}

pub fn scan_cursor_state_db_workspace(workspace: &Path) -> Vec<(SessionRecord, Vec<Event>)> {
    db_paths(workspace)
        .into_iter()
        .flat_map(|path| scan_db(&path, workspace).unwrap_or_default())
        .collect()
}

fn open_read_only(path: &Path) -> Result<Connection> {
    Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .with_context(|| format!("open cursor state db read-only: {}", path.display()))
}

fn db_paths(workspace: &Path) -> Vec<PathBuf> {
    [
        std::env::var("CURSOR_STATE_DB").ok().map(PathBuf::from),
        Some(workspace.join(".cursor/state.vscdb")),
        Some(workspace.join("state.vscdb")),
    ]
    .into_iter()
    .flatten()
    .filter(|path| path.is_file())
    .collect()
}

fn scan_db(path: &Path, workspace: &Path) -> Result<Vec<(SessionRecord, Vec<Event>)>> {
    Ok(read_items_with_prefix(path, "composerData:")?
        .into_iter()
        .filter_map(|item| session_from_item(item, path, workspace))
        .collect())
}

fn session_from_item(
    item: CursorStateItem,
    path: &Path,
    workspace: &Path,
) -> Option<(SessionRecord, Vec<Event>)> {
    let value: Value = serde_json::from_str(&item.value).ok()?;
    let root = workspace_field(&value).unwrap_or_else(|| workspace.to_string_lossy().into());
    workspace_matches(&root, workspace).then(|| record_and_events(item, value, path, root))
}

fn record_and_events(
    item: CursorStateItem,
    value: Value,
    path: &Path,
    workspace: String,
) -> (SessionRecord, Vec<Event>) {
    let id = id_field(&value).unwrap_or_else(|| key_suffix(&item.key));
    let started = ts_field(&value).unwrap_or_else(|| file_mtime_ms(path));
    let events = vec![event(&id, started, item.key, value.clone())];
    (record(id, value, path, workspace, started), events)
}

fn record(
    id: String,
    value: Value,
    path: &Path,
    workspace: String,
    started_at_ms: u64,
) -> SessionRecord {
    SessionRecord {
        id,
        agent: "cursor".into(),
        model: text(&value, "model"),
        workspace,
        started_at_ms,
        ended_at_ms: ts_any(&value, &["ended_at_ms", "updated_at_ms"]),
        status: SessionStatus::Done,
        trace_path: path.to_string_lossy().into(),
        start_commit: None,
        end_commit: None,
        branch: None,
        dirty_start: None,
        dirty_end: None,
        repo_binding_source: None,
        prompt_fingerprint: None,
        parent_session_id: None,
        agent_version: None,
        os: None,
        arch: None,
        repo_file_count: None,
        repo_total_loc: None,
    }
}

fn event(session_id: &str, ts_ms: u64, key: String, value: Value) -> Event {
    Event {
        session_id: session_id.into(),
        seq: 0,
        ts_ms,
        ts_exact: ts_field(&value).is_some(),
        kind: EventKind::Message,
        source: EventSource::Tail,
        tool: None,
        tool_call_id: None,
        tokens_in: None,
        tokens_out: None,
        reasoning_tokens: None,
        cost_usd_e6: None,
        stop_reason: None,
        latency_ms: None,
        ttft_ms: None,
        retry_count: None,
        context_used_tokens: None,
        context_max_tokens: None,
        cache_creation_tokens: None,
        cache_read_tokens: None,
        system_prompt_tokens: None,
        payload: json!({"cursor_state_key": key, "value": value}),
    }
}

fn has_item_table(conn: &Connection) -> Result<bool> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='ItemTable'",
        [],
        |row| row.get(0),
    )?;
    Ok(n > 0)
}

fn row_item(row: &rusqlite::Row<'_>) -> rusqlite::Result<CursorStateItem> {
    Ok(CursorStateItem {
        key: row.get(0)?,
        value: value_text(row.get_ref(1)?),
    })
}

fn value_text(value: ValueRef<'_>) -> String {
    match value {
        ValueRef::Null => String::new(),
        ValueRef::Integer(v) => v.to_string(),
        ValueRef::Real(v) => v.to_string(),
        ValueRef::Text(v) | ValueRef::Blob(v) => String::from_utf8_lossy(v).into_owned(),
    }
}

fn escape_like(prefix: &str) -> String {
    prefix
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}
