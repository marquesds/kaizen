// SPDX-License-Identifier: AGPL-3.0-or-later
//! Workspace scope helpers for repo-local vs machine-wide reads.

use anyhow::Result;
use std::path::{Path, PathBuf};

/// How the active workspace was selected for a command invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopeOrigin {
    /// Derived from the process current working directory.
    Cwd,
    /// Explicit `--workspace <path>` flag.
    ExplicitWorkspace,
    /// Explicit `--project <name>` flag, resolved to a path.
    ExplicitProject(String),
}

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

/// Returns a scope annotation when the workspace selection might surprise the user.
///
/// Prints when an explicit flag was used, or when cwd maps to an unregistered path.
pub fn scope_header(ws: &Path, origin: &ScopeOrigin) -> Option<String> {
    match origin {
        ScopeOrigin::ExplicitWorkspace | ScopeOrigin::ExplicitProject(_) => {
            let name = ws.file_name().and_then(|n| n.to_str()).unwrap_or("?");
            Some(format!("scope: {} ({})", name, ws.display()))
        }
        ScopeOrigin::Cwd => {
            let registered = crate::core::machine_registry::list_paths()
                .unwrap_or_default()
                .into_iter()
                .any(|p| p == ws);
            if registered {
                None
            } else {
                Some(format!("scope: {} ({})", ws.display(), "unregistered cwd"))
            }
        }
    }
}
