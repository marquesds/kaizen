// SPDX-License-Identifier: AGPL-3.0-or-later
//! Shared path helpers (used by `workspace` and `machine_registry` to avoid import cycles).

use anyhow::Result;
use std::path::{Path, PathBuf};

/// `KAIZEN_HOME` or `~/.kaizen` (requires `HOME`), or `None` if undiscoverable.
pub fn kaizen_dir() -> Option<PathBuf> {
    std::env::var("KAIZEN_HOME")
        .ok()
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var("HOME")
                .ok()
                .map(|home| PathBuf::from(home).join(".kaizen"))
        })
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
    let home = kaizen_dir().ok_or_else(|| anyhow::anyhow!("KAIZEN_HOME / HOME unset"))?;
    let canon = std::fs::canonicalize(workspace).unwrap_or_else(|_| workspace.to_path_buf());
    let slug = workspace_slug(&canon);
    Ok(home.join("projects").join(slug))
}

/// Project data path, created on demand for write-capable callers.
pub fn project_data_dir(workspace: &Path) -> Result<PathBuf> {
    let dir = project_data_path(workspace)?;
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
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
