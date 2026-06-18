// SPDX-License-Identifier: AGPL-3.0-or-later
//! Copy legacy workspace data into Kaizen home without changing the workspace.

use anyhow::Result;
use std::path::Path;

const MARKER: &str = "LEGACY_IMPORTED.txt";

pub enum ImportOutcome {
    Skipped,
    AlreadyImported,
    Conflict,
    Imported,
}

/// Copy `<workspace>/.kaizen` into `target`; leave source bytes untouched.
pub fn import_legacy(workspace: &Path, target: &Path) -> Result<ImportOutcome> {
    let source = workspace.join(".kaizen");
    if let Some(outcome) = skip_reason(&source, target)? {
        return Ok(outcome);
    }
    validate_tree(&source)?;
    std::fs::create_dir_all(target)?;
    copy_entries(&source, target)?;
    write_marker(&source, target)?;
    Ok(ImportOutcome::Imported)
}

fn skip_reason(source: &Path, target: &Path) -> Result<Option<ImportOutcome>> {
    if source_missing(source)? {
        return Ok(Some(ImportOutcome::Skipped));
    }
    if target.join(MARKER).exists() {
        return Ok(Some(ImportOutcome::AlreadyImported));
    }
    Ok(target_has_data(target)?.then_some(ImportOutcome::Conflict))
}

fn source_missing(source: &Path) -> Result<bool> {
    match std::fs::symlink_metadata(source) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(true),
        Err(error) => Err(error.into()),
        Ok(_) => {
            anyhow::ensure!(
                validated_metadata(source)?.is_dir(),
                "legacy root must be a directory"
            );
            Ok(false)
        }
    }
}

fn target_has_data(target: &Path) -> Result<bool> {
    Ok(target.exists() && std::fs::read_dir(target)?.next().transpose()?.is_some())
}

fn copy_entries(source: &Path, target: &Path) -> Result<()> {
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        if entry.file_name() != "MIGRATED.txt" {
            copy_recursive(&entry.path(), &target.join(entry.file_name()))?;
        }
    }
    Ok(())
}

fn validate_tree(root: &Path) -> Result<()> {
    anyhow::ensure!(
        validated_metadata(root)?.is_dir(),
        "legacy root must be a directory"
    );
    for entry in std::fs::read_dir(root)? {
        validate_entry(&entry?.path())?;
    }
    Ok(())
}

fn validate_entry(path: &Path) -> Result<()> {
    let metadata = validated_metadata(path)?;
    if metadata.is_dir() {
        validate_tree(path)?;
    }
    Ok(())
}

fn validated_metadata(path: &Path) -> Result<std::fs::Metadata> {
    let metadata = std::fs::symlink_metadata(path)?;
    anyhow::ensure!(
        !metadata.file_type().is_symlink(),
        "legacy import rejects symlink: {}",
        path.display()
    );
    anyhow::ensure!(
        metadata.is_file() || metadata.is_dir(),
        "unsupported legacy entry: {}",
        path.display()
    );
    Ok(metadata)
}

fn copy_recursive(source: &Path, target: &Path) -> Result<()> {
    let metadata = validated_metadata(source)?;
    if !metadata.is_dir() {
        std::fs::copy(source, target)?;
        return Ok(());
    }
    std::fs::create_dir_all(target)?;
    copy_entries(source, target)
}

fn write_marker(source: &Path, target: &Path) -> Result<()> {
    let message = format!(
        "copied from: {}\nsource left unchanged; remove it manually after verification\n",
        source.display()
    );
    std::fs::write(target.join(MARKER), message)?;
    Ok(())
}
