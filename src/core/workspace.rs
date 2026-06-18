// SPDX-License-Identifier: AGPL-3.0-or-later
//! Workspace path canonicalization + machine-local registry.

use anyhow::Result;
use std::path::{Path, PathBuf};

pub use crate::core::paths::{canonical, kaizen_dir, project_data_dir};

pub fn resolve(path: Option<&Path>) -> Result<PathBuf> {
    let canonical = resolve_read(path)?;
    register_workspace(&canonical);
    Ok(canonical)
}

/// Resolve an existing workspace without creating registry or project state.
pub fn resolve_read(path: Option<&Path>) -> Result<PathBuf> {
    let root = path
        .map(Path::to_path_buf)
        .map(Ok)
        .unwrap_or_else(std::env::current_dir)?;
    canonical_directory(&root)
}

fn canonical_directory(path: &Path) -> Result<PathBuf> {
    let canonical = std::fs::canonicalize(path).map_err(|error| canonicalize_error(path, error))?;
    anyhow::ensure!(
        canonical.is_dir(),
        "workspace is not a directory: {}",
        path.display()
    );
    Ok(canonical)
}

fn canonicalize_error(path: &Path, error: std::io::Error) -> anyhow::Error {
    if error.kind() == std::io::ErrorKind::NotFound {
        return anyhow::anyhow!("workspace does not exist: {}", path.display());
    }
    anyhow::Error::new(error).context(format!("canonicalize workspace: {}", path.display()))
}

fn register_workspace(workspace: &Path) {
    let Ok(data_dir) = project_data_dir(workspace) else {
        return;
    };
    let _ = crate::core::machine_registry::upsert_from_resolve(workspace);
    if let Err(e) = crate::core::legacy_import::import_legacy(workspace, &data_dir) {
        tracing::warn!("legacy import failed: {e}");
    }
}

pub fn machine_workspaces(seed: Option<&Path>) -> Result<Vec<PathBuf>> {
    let seed = seed.map(canonical);
    let mut roots = registry_entries()?;
    if let Some(path) = seed.as_ref() {
        push_unique(&mut roots, path.clone());
    }
    roots.retain(|path| seed.as_ref() == Some(path) || usable_registered_workspace(path));
    if roots.is_empty()
        && let Some(path) = seed
    {
        roots.push(path);
    }
    Ok(roots)
}

fn usable_registered_workspace(path: &Path) -> bool {
    path.exists()
        && db_path(path)
            .is_ok_and(|db| db.exists() || crate::core::machine_registry::is_registered(path))
}

pub fn db_path(workspace: &Path) -> Result<PathBuf> {
    let path = crate::core::paths::project_data_child(workspace, Path::new("kaizen.db"))?;
    ["kaizen.db-journal", "kaizen.db-wal", "kaizen.db-shm"]
        .into_iter()
        .try_for_each(|name| {
            crate::core::paths::project_data_child(workspace, Path::new(name)).map(drop)
        })?;
    Ok(path)
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
            "unknown project '{name}'. run 'kaizen projects' to see registered projects."
        ),
        _ => Err(ambiguous_error(name, &segs)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::paths::test_lock;
    use std::path::Path;
    use tempfile::TempDir;

    fn with_home<T>(test: impl FnOnce(&Path) -> T) -> T {
        let _guard = test_lock::global().lock().unwrap();
        let home = TempDir::new().unwrap();
        unsafe { std::env::set_var("KAIZEN_HOME", home.path().join(".kaizen")) };
        let result = test(home.path());
        unsafe { std::env::remove_var("KAIZEN_HOME") };
        result
    }

    #[test]
    fn registry_round_trip() {
        with_home(|home| {
            let ws = home.join("repo");
            std::fs::create_dir_all(&ws).unwrap();
            let first = resolve(Some(&ws)).unwrap();
            assert_eq!(first, std::fs::canonicalize(ws).unwrap());
            assert!(crate::core::machine_registry::is_registered(&first));
        });
    }

    #[test]
    fn resolve_rejects_non_directory_without_state() {
        with_home(|home| {
            let file = home.join("workspace-file");
            std::fs::write(&file, "not a directory").unwrap();
            let error = resolve(Some(&file)).unwrap_err().to_string();
            assert!(error.contains("workspace is not a directory"), "{error}");
            assert!(!home.join(".kaizen").exists());
        });
    }

    #[cfg(unix)]
    #[test]
    fn resolve_preserves_non_missing_canonicalize_error() {
        with_home(|home| {
            let loop_path = home.join("loop");
            std::os::unix::fs::symlink(&loop_path, &loop_path).unwrap();
            let error = resolve(Some(&loop_path)).unwrap_err().to_string();
            assert!(error.contains("canonicalize workspace"), "{error}");
            assert!(!error.contains("does not exist"), "{error}");
            assert!(!home.join(".kaizen").exists());
        });
    }

    #[test]
    fn resolve_project_name_no_match() {
        with_home(|_| {
            let err = resolve_project_name("nonexistent").unwrap_err();
            assert!(err.to_string().contains("unknown project"));
        });
    }
}
