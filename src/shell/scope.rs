// SPDX-License-Identifier: AGPL-3.0-or-later
//! Workspace scope helpers for repo-local vs machine-wide reads.

use anyhow::Result;
use std::path::{Path, PathBuf};

pub fn resolve(workspace: Option<&Path>, all_workspaces: bool) -> Result<Vec<PathBuf>> {
    let primary = crate::core::workspace::resolve(workspace)?;
    if all_workspaces {
        return crate::core::workspace::machine_workspaces(Some(&primary));
    }
    Ok(vec![primary])
}

pub fn label(roots: &[PathBuf]) -> String {
    if roots.len() == 1 {
        return roots[0].to_string_lossy().to_string();
    }
    format!("machine:{} workspaces", roots.len())
}

pub fn decorate_path(workspace: &Path, path: &str) -> String {
    format!("{}:{}", workspace.to_string_lossy(), path)
}
