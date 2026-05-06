// SPDX-License-Identifier: AGPL-3.0-or-later
//! Workspace path canonicalization + machine-local registry.

use anyhow::Result;
use std::path::{Path, PathBuf};

pub use crate::core::paths::{canonical, kaizen_dir, project_data_dir};

pub fn resolve(path: Option<&Path>) -> Result<PathBuf> {
    let root = path
        .map(Path::to_path_buf)
        .map(Ok)
        .unwrap_or_else(std::env::current_dir)?;
    let canonical = canonical(&root);
    let _ = crate::core::machine_registry::upsert_from_resolve(&canonical);
    if let Ok(data_dir) = project_data_dir(&canonical)
        && let Err(e) = crate::core::migrate_home::migrate_legacy_in_repo(&canonical, &data_dir)
    {
        tracing::warn!("legacy migration failed: {e}");
    }
    Ok(canonical)
}

pub fn machine_workspaces(seed: Option<&Path>) -> Result<Vec<PathBuf>> {
    let seed = seed.map(canonical);
    let mut roots = registry_entries()?;
    if let Some(path) = seed.as_ref() {
        push_unique(&mut roots, path.clone());
    }
    roots.retain(|p| {
        if seed.as_ref() == Some(p) {
            return true;
        }
        p.exists()
            && (db_path(p).ok().is_some_and(|d| d.exists())
                || crate::core::machine_registry::is_registered(p))
    });
    if roots.is_empty()
        && let Some(path) = seed
    {
        roots.push(path);
    }
    Ok(roots)
}

pub fn db_path(workspace: &Path) -> Result<PathBuf> {
    Ok(project_data_dir(workspace)?.join("kaizen.db"))
}

fn registry_entries() -> Result<Vec<PathBuf>> {
    crate::core::machine_registry::list_paths()
}

fn push_unique(roots: &mut Vec<PathBuf>, path: PathBuf) {
    if !roots.iter().any(|row| row == &path) {
        roots.push(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::paths::test_lock;
    use tempfile::TempDir;

    #[test]
    fn registry_round_trip() {
        let _guard = test_lock::global().lock().unwrap();
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
