// SPDX-License-Identifier: AGPL-3.0-or-later
//! Workspace path canonicalization + machine-local registry.

use anyhow::Result;
use std::path::{Path, PathBuf};

const REGISTRY_FILE: &str = "workspaces.json";

pub fn resolve(path: Option<&Path>) -> Result<PathBuf> {
    let root = path
        .map(Path::to_path_buf)
        .map(Ok)
        .unwrap_or_else(std::env::current_dir)?;
    let canonical = canonical(&root);
    register(&canonical)?;
    Ok(canonical)
}

pub fn machine_workspaces(seed: Option<&Path>) -> Result<Vec<PathBuf>> {
    let seed = seed.map(canonical);
    let mut roots = registry_entries()?;
    if let Some(path) = seed.as_ref() {
        push_unique(&mut roots, path.clone());
    }
    roots.retain(|path| db_path(path).exists() || seed.as_ref() == Some(path));
    if roots.is_empty()
        && let Some(path) = seed
    {
        roots.push(path);
    }
    Ok(roots)
}

pub fn db_path(workspace: &Path) -> PathBuf {
    workspace.join(".kaizen/kaizen.db")
}

fn register(workspace: &Path) -> Result<()> {
    let mut roots = registry_entries()?;
    push_unique(&mut roots, workspace.to_path_buf());
    write_registry(&roots)
}

fn registry_entries() -> Result<Vec<PathBuf>> {
    let Some(path) = registry_path() else {
        return Ok(Vec::new());
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return Ok(Vec::new());
    };
    let rows = serde_json::from_str::<Vec<String>>(&text).unwrap_or_default();
    Ok(rows.into_iter().map(PathBuf::from).collect())
}

fn write_registry(roots: &[PathBuf]) -> Result<()> {
    let Some(path) = registry_path() else {
        return Ok(());
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let rows = roots
        .iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect::<Vec<_>>();
    let body = serde_json::to_string_pretty(&rows)?;
    std::fs::write(path, body)?;
    Ok(())
}

fn registry_path() -> Option<PathBuf> {
    kaizen_home().map(|path| path.join(REGISTRY_FILE))
}

fn kaizen_home() -> Option<PathBuf> {
    std::env::var("KAIZEN_HOME")
        .ok()
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var("HOME")
                .ok()
                .map(|home| PathBuf::from(home).join(".kaizen"))
        })
}

fn push_unique(roots: &mut Vec<PathBuf>, path: PathBuf) {
    if !roots.iter().any(|row| row == &path) {
        roots.push(path);
    }
}

pub fn canonical(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| absolute(path))
}

fn absolute(path: &Path) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }
    std::env::current_dir()
        .map(|cwd| cwd.join(path))
        .unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};
    use tempfile::TempDir;

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn registry_round_trip() {
        let _guard = env_lock().lock().unwrap();
        let home = TempDir::new().unwrap();
        let ws = home.path().join("repo");
        std::fs::create_dir_all(&ws).unwrap();
        unsafe { std::env::set_var("KAIZEN_HOME", home.path().join(".kaizen")) };
        let first = resolve(Some(&ws)).unwrap();
        let rows = machine_workspaces(Some(&first)).unwrap();
        assert_eq!(rows, vec![first]);
        unsafe { std::env::remove_var("KAIZEN_HOME") };
    }
}
