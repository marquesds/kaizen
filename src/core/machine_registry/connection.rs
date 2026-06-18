use anyhow::{Context, Result};
use rusqlite::{Connection, OpenFlags};

use super::{db_path, sql};

pub(super) fn open_write() -> Result<Option<Connection>> {
    let Some(path) = db_path() else {
        return Ok(None);
    };
    create_parent(&path)?;
    let conn = open(&path)?;
    migrate(&conn)?;
    Ok(Some(conn))
}

pub(super) fn open_read() -> Result<Option<Connection>> {
    let Some(path) = db_path() else {
        return Ok(None);
    };
    if !path.exists() {
        return Ok(None);
    }
    open_read_path(&path).map(Some)
}

fn open_read_path(path: &std::path::Path) -> Result<Connection> {
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let conn = Connection::open_with_flags(path, flags)
        .with_context(|| format!("open machine registry: {}", path.display()))?;
    conn.execute_batch("PRAGMA query_only=ON; PRAGMA busy_timeout=5000;")?;
    Ok(conn)
}

fn create_parent(path: &std::path::Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn open(path: &std::path::Path) -> Result<Connection> {
    let conn = Connection::open(path)
        .with_context(|| format!("open machine registry: {}", path.display()))?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")?;
    Ok(conn)
}

fn migrate(conn: &Connection) -> Result<()> {
    sql::MIGRATIONS.iter().try_for_each(|statement| {
        conn.execute_batch(statement)
            .with_context(|| format!("machine registry migration: {statement}"))
    })
}
