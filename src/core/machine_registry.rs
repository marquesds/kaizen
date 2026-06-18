// SPDX-License-Identifier: AGPL-3.0-or-later
//! Machine-local SQLite registry (`$KAIZEN_HOME/machine.db`) — known workspace roots, init history.

use anyhow::{Context, Result};
use rusqlite::{Connection, params};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::core::paths::{canonical, kaizen_dir};

mod connection;
mod legacy;
mod metadata;
mod sql;

const MACHINE_DB: &str = "machine.db";

/// Path to the machine registry db, or `None` if `KAIZEN_HOME` / `HOME` is unset.
pub fn db_path() -> Option<PathBuf> {
    kaizen_dir().map(|d| d.join(MACHINE_DB))
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn name_for_path(path: &Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(str::to_string)
        .unwrap_or_default()
}

fn open_conn_write() -> Result<Option<Connection>> {
    connection::open_write()
}

fn open_conn_read() -> Result<Option<Connection>> {
    connection::open_read()
}

fn with_write<F>(f: F) -> Result<()>
where
    F: FnOnce(&Connection) -> Result<()>,
{
    let Some(conn) = open_conn_write()? else {
        return Ok(());
    };
    legacy::migrate(&conn)?;
    f(&conn)
}

/// Upsert a workspace seen from [`resolve`](crate::core::workspace::resolve).
pub fn upsert_from_resolve(path: &Path) -> Result<()> {
    with_write(|conn| upsert_seen(conn, path))
}

fn upsert_seen(conn: &Connection, path: &Path) -> Result<()> {
    let c = canonical(path);
    let t = now_ms();
    let name = name_for_path(&c);
    let p = c.to_string_lossy();
    conn.execute(sql::UPSERT_SEEN, params![p.as_ref(), &name, t, t])
        .context("machine registry upsert from resolve")?;
    Ok(())
}

/// Record a successful `kaizen init` (increments `init_count`, optional git + version).
pub fn record_init(path: &Path) -> Result<()> {
    with_write(|conn| insert_init(conn, path))
}

fn insert_init(conn: &Connection, path: &Path) -> Result<()> {
    let c = canonical(path);
    let t = now_ms();
    let name = name_for_path(&c);
    let p = c.to_string_lossy();
    let origin = metadata::git_remote_origin(&c);
    let values = params![
        p.as_ref(),
        &name,
        t,
        t,
        t,
        origin.as_deref(),
        env!("CARGO_PKG_VERSION")
    ];
    conn.execute(sql::RECORD_INIT, values)
        .context("machine registry record init")?;
    Ok(())
}

/// All known workspace paths from the machine registry.
pub fn list_paths() -> Result<Vec<PathBuf>> {
    Ok(list_paths_including_missing()?
        .into_iter()
        .filter(|path| path.is_dir())
        .collect())
}

/// All registry rows, including workspace paths that no longer exist.
pub fn list_paths_including_missing() -> Result<Vec<PathBuf>> {
    let mut paths = legacy::read_paths();
    if let Some(conn) = open_conn_read()? {
        extend_unique(&mut paths, query_paths(&conn)?);
    }
    Ok(paths)
}

fn extend_unique(paths: &mut Vec<PathBuf>, additions: impl IntoIterator<Item = PathBuf>) {
    additions.into_iter().for_each(|path| {
        if !paths.contains(&path) {
            paths.push(path);
        }
    });
}

fn query_paths(conn: &Connection) -> Result<Vec<PathBuf>> {
    let mut stmt = conn
        .prepare(sql::LIST_PATHS)
        .context("machine registry list paths")?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0).map(PathBuf::from))
        .context("query machine registry")?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// `true` if this path is a row in the machine registry (compared after canonicalize).
pub fn is_registered(path: &Path) -> bool {
    let canonical = canonical(path);
    if legacy::read_paths().contains(&canonical) {
        return true;
    }
    let Some(conn) = open_conn_read().ok().flatten() else {
        return false;
    };
    registered(&conn, &canonical)
}

fn registered(conn: &Connection, path: &Path) -> bool {
    let c = canonical(path);
    let p = c.to_string_lossy();
    conn.query_row(sql::IS_REGISTERED, [p.as_ref()], |_| Ok(()))
        .is_ok()
}

/// Read machine registry status without creating machine state.
pub fn status() -> Result<Option<(PathBuf, usize)>> {
    let Some(path) = db_path() else {
        return Ok(None);
    };
    let legacy = legacy::read_paths();
    let conn = open_conn_read()?;
    Ok(Some((path, total_project_count(conn.as_ref(), &legacy))))
}

fn total_project_count(conn: Option<&Connection>, legacy: &[PathBuf]) -> usize {
    match conn {
        Some(conn) => project_count(conn) + legacy.iter().filter(|p| !registered(conn, p)).count(),
        None => legacy.len(),
    }
}

fn project_count(conn: &Connection) -> usize {
    conn.query_row(sql::PROJECT_COUNT, [], |row| row.get::<_, i64>(0))
        .unwrap_or(0) as usize
}
