// SPDX-License-Identifier: AGPL-3.0-or-later
//! Validated user-home paths for Kaizen-owned writes.

use anyhow::Result;
use std::path::{Path, PathBuf};

pub fn root(workspace: &Path) -> Result<PathBuf> {
    let home =
        super::paths::kaizen_dir().ok_or_else(|| anyhow::anyhow!("KAIZEN_HOME / HOME unset"))?;
    anyhow::ensure!(
        !super::paths::path_is_within(&home, workspace),
        "KAIZEN_HOME must be outside target repository"
    );
    Ok(home)
}

pub fn file_for_write(workspace: &Path, relative: &Path) -> Result<PathBuf> {
    let home = root(workspace)?;
    std::fs::create_dir_all(&home)?;
    anyhow::ensure!(
        !super::paths::path_is_within(&home, workspace),
        "KAIZEN_HOME must be outside target repository"
    );
    super::paths::descendant_file_for_write(&home, relative)
}

pub fn sqlite_file_for_write(workspace: &Path, name: &str) -> Result<PathBuf> {
    let path = file_for_write(workspace, Path::new(name))?;
    ["-journal", "-wal", "-shm"]
        .into_iter()
        .try_for_each(|suffix| {
            let sidecar = format!("{name}{suffix}");
            super::paths::descendant_path(path.parent().unwrap(), Path::new(&sidecar)).map(drop)
        })?;
    Ok(path)
}
