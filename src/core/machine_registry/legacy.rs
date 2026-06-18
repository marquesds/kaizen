use anyhow::Result;
use rusqlite::{Connection, params};
use std::path::{Path, PathBuf};

use crate::core::paths::{canonical, kaizen_dir};

use super::{name_for_path, now_ms, sql};

const LEGACY_WORKSPACES_JSON: &str = "workspaces.json";

pub(super) fn migrate(conn: &Connection) -> Result<()> {
    let Some((home, legacy)) = legacy_paths() else {
        return Ok(());
    };
    let seen_at = now_ms();
    read_rows(&legacy)
        .into_iter()
        .filter(|path| path.exists())
        .for_each(|path| upsert(conn, &path, seen_at));
    archive(&home, &legacy);
    Ok(())
}

pub(super) fn read_paths() -> Vec<PathBuf> {
    let Some((_, legacy)) = legacy_paths() else {
        return Vec::new();
    };
    read_rows(&legacy)
        .into_iter()
        .filter(|path| path.exists())
        .map(|path| canonical(&path))
        .collect()
}

fn legacy_paths() -> Option<(PathBuf, PathBuf)> {
    let home = kaizen_dir()?;
    let legacy = home.join(LEGACY_WORKSPACES_JSON);
    legacy.exists().then_some((home, legacy))
}

fn read_rows(path: &Path) -> Vec<PathBuf> {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    serde_json::from_str::<Vec<String>>(&text)
        .unwrap_or_default()
        .into_iter()
        .map(PathBuf::from)
        .collect()
}

fn upsert(conn: &Connection, path: &Path, seen_at: i64) {
    let canonical = canonical(path);
    let name = name_for_path(&canonical);
    let path = canonical.to_string_lossy();
    let values = params![path.as_ref(), &name, seen_at, seen_at];
    let _ = conn.execute(sql::IMPORT_LEGACY, values);
}

fn archive(home: &Path, legacy: &Path) {
    let migrated = home.join("workspaces.json.migrated");
    if std::fs::rename(legacy, migrated).is_err() {
        let _ = std::fs::remove_file(legacy);
    }
}
