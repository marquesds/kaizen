// SPDX-License-Identifier: AGPL-3.0-or-later
//! Machine-local SQLite registry (`$KAIZEN_HOME/machine.db`) — known workspace roots, init history.

use anyhow::{Context, Result};
use rusqlite::{Connection, params};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::core::paths::{canonical, kaizen_dir};

const MACHINE_DB: &str = "machine.db";
const LEGACY_WORKSPACES_JSON: &str = "workspaces.json";

const MIGRATIONS: &[&str] = &["CREATE TABLE IF NOT EXISTS projects (
        path TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        first_seen_ms INTEGER NOT NULL,
        last_seen_ms INTEGER NOT NULL,
        last_init_ms INTEGER,
        init_count INTEGER NOT NULL DEFAULT 0,
        git_remote_origin TEXT,
        kaizen_version_at_init TEXT,
        meta TEXT
    )"];

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
    let Some(path) = db_path() else {
        return Ok(None);
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(&path)
        .with_context(|| format!("open machine registry: {}", path.display()))?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")?;
    for sql in MIGRATIONS {
        conn.execute_batch(sql)
            .with_context(|| format!("machine registry migration: {sql}"))?;
    }
    Ok(Some(conn))
}

/// Import legacy `workspaces.json` if present, then rename it.
fn migrate_legacy_workspaces_json(conn: &Connection) -> Result<()> {
    let Some(home) = kaizen_dir() else {
        return Ok(());
    };
    let legacy = home.join(LEGACY_WORKSPACES_JSON);
    if !legacy.exists() {
        return Ok(());
    }
    let text = std::fs::read_to_string(&legacy).unwrap_or_default();
    let rows: Vec<String> = serde_json::from_str(&text).unwrap_or_default();
    let t = now_ms();
    for s in rows {
        let p = PathBuf::from(&s);
        if p.exists() {
            let c = canonical(&p);
            let name = name_for_path(&c);
            let _ = conn.execute(
                "INSERT INTO projects (path, name, first_seen_ms, last_seen_ms, last_init_ms, init_count, git_remote_origin, kaizen_version_at_init, meta)
                 VALUES (?1, ?2, ?3, ?4, NULL, 0, NULL, NULL, NULL)
                 ON CONFLICT(path) DO UPDATE SET
                   last_seen_ms = MAX(projects.last_seen_ms, excluded.last_seen_ms),
                   name = excluded.name",
                params![c.to_string_lossy().as_ref(), &name, t, t,],
            );
        }
    }
    let migrated = home.join("workspaces.json.migrated");
    if std::fs::rename(&legacy, &migrated).is_err() {
        let _ = std::fs::remove_file(&legacy);
    }
    Ok(())
}

fn with_write<F>(f: F) -> Result<()>
where
    F: FnOnce(&Connection) -> Result<()>,
{
    let Some(conn) = open_conn_write()? else {
        return Ok(());
    };
    migrate_legacy_workspaces_json(&conn)?;
    f(&conn)
}

/// Upsert a workspace seen from [`resolve`](crate::core::workspace::resolve).
pub fn upsert_from_resolve(path: &Path) -> Result<()> {
    with_write(|conn| {
        let c = canonical(path);
        let t = now_ms();
        let name = name_for_path(&c);
        let p = c.to_string_lossy();
        conn.execute(
            "INSERT INTO projects (path, name, first_seen_ms, last_seen_ms, last_init_ms, init_count, git_remote_origin, kaizen_version_at_init, meta)
             VALUES (?1, ?2, ?3, ?4, NULL, 0, NULL, NULL, NULL)
             ON CONFLICT(path) DO UPDATE SET
               name = excluded.name,
               last_seen_ms = MAX(projects.last_seen_ms, excluded.last_seen_ms),
               first_seen_ms = projects.first_seen_ms",
            params![p.as_ref(), &name, t, t],
        )
        .context("machine registry upsert from resolve")?;
        Ok(())
    })
}

/// Record a successful `kaizen init` (increments `init_count`, optional git + version).
pub fn record_init(path: &Path) -> Result<()> {
    with_write(|conn| {
        let c = canonical(path);
        let t = now_ms();
        let name = name_for_path(&c);
        let p = c.to_string_lossy();
        let ver = env!("CARGO_PKG_VERSION");
        let origin = git_remote_origin(&c);
        let origin_ref = origin.as_deref();
        conn.execute(
            "INSERT INTO projects (path, name, first_seen_ms, last_seen_ms, last_init_ms, init_count, git_remote_origin, kaizen_version_at_init, meta)
             VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?7, NULL)
             ON CONFLICT(path) DO UPDATE SET
               name = excluded.name,
               last_seen_ms = MAX(projects.last_seen_ms, excluded.last_seen_ms),
               last_init_ms = excluded.last_init_ms,
               init_count = projects.init_count + 1,
               git_remote_origin = COALESCE(excluded.git_remote_origin, projects.git_remote_origin),
               kaizen_version_at_init = excluded.kaizen_version_at_init,
               first_seen_ms = projects.first_seen_ms",
            params![p.as_ref(), &name, t, t, t, origin_ref, ver],
        )
        .context("machine registry record init")?;
        Ok(())
    })
}

fn git_remote_origin(repo: &Path) -> Option<String> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["remote", "get-url", "origin"])
        .output()
        .ok()?;
    if out.status.success() {
        return String::from_utf8(out.stdout)
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
    }
    None
}

/// All known workspace paths from the machine registry.
pub fn list_paths() -> Result<Vec<PathBuf>> {
    let Some(conn) = open_conn_write()? else {
        return Ok(Vec::new());
    };
    migrate_legacy_workspaces_json(&conn)?;
    let mut stmt = conn
        .prepare("SELECT path FROM projects ORDER BY last_seen_ms DESC")
        .context("machine registry list paths")?;
    let rows = stmt
        .query_map([], |r| {
            let s: String = r.get(0)?;
            Ok(PathBuf::from(s))
        })
        .context("query machine registry")?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// `true` if this path is a row in the machine registry (compared after canonicalize).
pub fn is_registered(path: &Path) -> bool {
    let Some(conn) = open_conn_write().ok().flatten() else {
        return false;
    };
    if migrate_legacy_workspaces_json(&conn).is_err() {
        return false;
    }
    let c = canonical(path);
    let p = c.to_string_lossy();
    conn.query_row(
        "SELECT 1 FROM projects WHERE path = ?1",
        [p.as_ref()],
        |_| Ok(()),
    )
    .is_ok()
}

/// Open machine registry (read/write), run migrations, return project count, or `None` if no kaizen home.
pub fn status() -> Result<Option<(PathBuf, usize)>> {
    let Some(path) = db_path() else {
        return Ok(None);
    };
    let Some(conn) = open_conn_write()? else {
        return Ok(None);
    };
    migrate_legacy_workspaces_json(&conn)?;
    let n: i64 = conn
        .query_row("SELECT COUNT(*) FROM projects", [], |r| r.get(0))
        .unwrap_or(0);
    Ok(Some((path, n as usize)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::paths::test_lock;

    #[test]
    fn upsert_and_list() {
        let _g = test_lock::global().lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join(".kaizen");
        std::fs::create_dir_all(&home).unwrap();
        unsafe { std::env::set_var("KAIZEN_HOME", &home) };
        let ws = tmp.path().join("r");
        std::fs::create_dir_all(&ws).unwrap();
        let ws = std::fs::canonicalize(&ws).unwrap();
        upsert_from_resolve(&ws).unwrap();
        let paths = list_paths().unwrap();
        assert_eq!(paths, vec![ws]);
        assert!(is_registered(&paths[0]));
        unsafe { std::env::remove_var("KAIZEN_HOME") };
    }
}
