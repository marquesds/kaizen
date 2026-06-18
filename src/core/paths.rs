// SPDX-License-Identifier: AGPL-3.0-or-later
//! Shared path helpers (used by `workspace` and `machine_registry` to avoid import cycles).

use anyhow::Result;
use std::path::{Component, Path, PathBuf};

/// `KAIZEN_HOME` or `~/.kaizen` (requires `HOME`), or `None` if undiscoverable.
pub fn kaizen_dir() -> Option<PathBuf> {
    let configured = std::env::var("KAIZEN_HOME")
        .ok()
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var("HOME")
                .ok()
                .map(|home| PathBuf::from(home).join(".kaizen"))
        });
    configured.map(|path| absolute(&path))
}

/// `/Users/lucas/Projects/kaizen` → `Users-lucas-Projects-kaizen`
///
/// Used for kaizen's own data dir (`~/.kaizen/projects/<slug>/`).
pub fn workspace_slug(path: &Path) -> String {
    path.to_string_lossy()
        .trim_start_matches('/')
        .replace('/', "-")
}

/// Cursor project slug: strips leading `/`, then replaces `/` and `.` with `-`.
///
/// Cursor stores transcripts at `~/.cursor/projects/<cursor_slug>/agent-transcripts`.
/// Example: `/Users/lucas.marques/Projects/kaizen` → `Users-lucas-marques-Projects-kaizen`.
pub fn cursor_slug(path: &Path) -> String {
    path.to_string_lossy()
        .trim_start_matches('/')
        .replace(['/', '.'], "-")
}

/// Claude Code project slug: leading `/` becomes `-`, then `/` and `.` → `-`.
///
/// Claude Code stores sessions at `~/.claude/projects/<claude_slug>/sessions`.
/// Example: `/Users/lucas.marques/Projects/kaizen` → `-Users-lucas-marques-Projects-kaizen`.
pub fn claude_code_slug(path: &Path) -> String {
    let s = path.to_string_lossy();
    let with_leading = if let Some(rest) = s.strip_prefix('/') {
        format!("-{rest}")
    } else {
        s.into_owned()
    };
    with_leading.replace(['/', '.'], "-")
}

/// `~/.kaizen/projects/<slug>/` (or `$KAIZEN_HOME/projects/<slug>/`) without I/O.
pub fn project_data_path(workspace: &Path) -> Result<PathBuf> {
    let home = crate::core::home_paths::root(workspace)?;
    let canon = std::fs::canonicalize(workspace).unwrap_or_else(|_| workspace.to_path_buf());
    let slug = workspace_slug(&canon);
    let data = home.join("projects").join(slug);
    ensure_project_data_outside_workspace(&data, &canon)?;
    Ok(data)
}

fn ensure_project_data_outside_workspace(data: &Path, workspace: &Path) -> Result<()> {
    ensure_outside_workspace(data, workspace, "Kaizen project data")
}

fn ensure_outside_workspace(path: &Path, workspace: &Path, label: &str) -> Result<()> {
    anyhow::ensure!(
        !path_is_within(path, workspace),
        "{label} must be outside target repository"
    );
    Ok(())
}

pub(crate) fn path_is_within(path: &Path, root: &Path) -> bool {
    let root = canonical(root);
    path.starts_with(&root)
        || path
            .ancestors()
            .find_map(|ancestor| ancestor.canonicalize().ok())
            .is_some_and(|ancestor| ancestor.starts_with(root))
}

/// Project data path, created on demand for write-capable callers.
pub fn project_data_dir(workspace: &Path) -> Result<PathBuf> {
    let dir = project_data_path(workspace)?;
    std::fs::create_dir_all(&dir)?;
    ensure_project_data_outside_workspace(&dir, &canonical(workspace))?;
    Ok(dir)
}

/// Existing project-data child path with symlink and traversal rejection.
pub fn project_data_child(workspace: &Path, relative: &Path) -> Result<PathBuf> {
    descendant_path(&project_data_path(workspace)?, relative)
}

/// Project-data directory prepared for a write.
pub fn project_dir_for_write(workspace: &Path, relative: &Path) -> Result<PathBuf> {
    descendant_dir_for_write(&project_data_dir(workspace)?, relative)
}

/// Project-data file path whose parent is prepared for a write.
pub fn project_file_for_write(workspace: &Path, relative: &Path) -> Result<PathBuf> {
    descendant_file_for_write(&project_data_dir(workspace)?, relative)
}

pub fn descendant_path(root: &Path, relative: &Path) -> Result<PathBuf> {
    ensure_relative(relative)?;
    let path = root.join(relative);
    ensure_no_symlinks(root, &path)?;
    Ok(path)
}

pub fn descendant_dir_for_write(root: &Path, relative: &Path) -> Result<PathBuf> {
    let path = descendant_path(root, relative)?;
    std::fs::create_dir_all(&path)?;
    ensure_no_symlinks(root, &path)?;
    Ok(path)
}

pub fn descendant_file_for_write(root: &Path, relative: &Path) -> Result<PathBuf> {
    let path = descendant_path(root, relative)?;
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("file path has no parent"))?;
    let relative_parent = parent.strip_prefix(root)?;
    if !relative_parent.as_os_str().is_empty() {
        descendant_dir_for_write(root, relative_parent)?;
    }
    ensure_no_symlinks(root, &path)?;
    Ok(path)
}

fn ensure_relative(path: &Path) -> Result<()> {
    let invalid = path
        .components()
        .any(|part| !matches!(part, Component::Normal(_)));
    anyhow::ensure!(
        !path.as_os_str().is_empty() && !invalid,
        "project path must be relative without traversal"
    );
    Ok(())
}

fn ensure_no_symlinks(root: &Path, path: &Path) -> Result<()> {
    let relative = path.strip_prefix(root)?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        current.push(component);
        validate_component(&current)?;
    }
    Ok(())
}

fn validate_component(path: &Path) -> Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => {
            anyhow::ensure!(
                !metadata.file_type().is_symlink(),
                "project data rejects symlink: {}",
                path.display()
            );
            crate::core::safe_fs::reject_hardlink(path)?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    Ok(())
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
pub(crate) mod test_lock {
    use std::sync::{Mutex, OnceLock};

    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    pub fn global() -> &'static Mutex<()> {
        LOCK.get_or_init(|| Mutex::new(()))
    }
}
