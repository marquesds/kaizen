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

fn slug_match(paths: &[PathBuf], name: &str) -> Vec<PathBuf> {
    paths
        .iter()
        .filter(|p| crate::core::paths::workspace_slug(p) == name)
        .cloned()
        .collect()
}

fn seg_match(paths: &[PathBuf], name: &str) -> Vec<PathBuf> {
    paths
        .iter()
        .filter(|p| p.file_name().and_then(|n| n.to_str()) == Some(name))
        .cloned()
        .collect()
}

fn ambiguous_error(name: &str, matches: &[PathBuf]) -> anyhow::Error {
    let list = matches
        .iter()
        .map(|p| format!("  {}", p.display()))
        .collect::<Vec<_>>()
        .join("\n");
    anyhow::anyhow!(
        "ambiguous project '{name}'. matches:\n{list}\nuse --workspace <path> or the slug."
    )
}

/// Resolve a short project name to its registered workspace path.
///
/// Resolution order:
/// 1. Exact slug match (`workspace_slug(path) == name`)
/// 2. Last path segment match (`path.file_name() == name`)
///
/// Returns `Err` on zero matches (unknown) or multiple matches (ambiguous).
pub fn resolve_project_name(name: &str) -> Result<PathBuf> {
    let paths = crate::core::machine_registry::list_paths()?;
    let slugs = slug_match(&paths, name);
    if slugs.len() == 1 {
        return Ok(slugs.into_iter().next().unwrap());
    }
    let segs = seg_match(&paths, name);
    match segs.len() {
        1 => Ok(segs.into_iter().next().unwrap()),
        0 => anyhow::bail!(
            "unknown project '{name}'. run 'kaizen projects list' to see registered projects."
        ),
        _ => Err(ambiguous_error(name, &segs)),
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

    #[test]
    fn resolve_project_name_no_match() {
        let _guard = test_lock::global().lock().unwrap();
        let home = TempDir::new().unwrap();
        unsafe { std::env::set_var("KAIZEN_HOME", home.path().join(".kaizen")) };
        let err = resolve_project_name("nonexistent").unwrap_err();
        assert!(err.to_string().contains("unknown project"));
        unsafe { std::env::remove_var("KAIZEN_HOME") };
    }
}
