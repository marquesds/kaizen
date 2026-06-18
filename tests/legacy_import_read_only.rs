// SPDX-License-Identifier: AGPL-3.0-or-later
//! Legacy import rejects links without touching source or destination.

#![cfg(unix)]

use std::path::{Path, PathBuf};

fn setup() -> anyhow::Result<(tempfile::TempDir, PathBuf, PathBuf, PathBuf)> {
    let tmp = tempfile::tempdir()?;
    let workspace = tmp.path().join("repo");
    let external = tmp.path().join("external");
    let target = tmp.path().join("home/project");
    std::fs::create_dir_all(&workspace)?;
    std::fs::create_dir_all(&external)?;
    std::fs::write(external.join("secret"), "outside\n")?;
    Ok((tmp, workspace, external, target))
}

fn assert_rejected(workspace: &Path, target: &Path) {
    let result = kaizen::core::legacy_import::import_legacy(workspace, target);
    assert!(result.is_err());
    assert!(!target.exists());
}

#[test]
fn legacy_import_rejects_symlink_without_copying_target() -> anyhow::Result<()> {
    let (_tmp, workspace, external, target) = setup()?;
    std::fs::create_dir_all(workspace.join(".kaizen"))?;
    std::os::unix::fs::symlink(&external, workspace.join(".kaizen/link"))?;

    assert_rejected(&workspace, &target);
    Ok(())
}

#[test]
fn legacy_import_rejects_symlinked_root() -> anyhow::Result<()> {
    let (_tmp, workspace, external, target) = setup()?;
    std::os::unix::fs::symlink(&external, workspace.join(".kaizen"))?;

    assert_rejected(&workspace, &target);
    Ok(())
}
