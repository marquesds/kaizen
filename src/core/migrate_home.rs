// SPDX-License-Identifier: AGPL-3.0-or-later
//! One-shot, idempotent migration: `<workspace>/.kaizen/` → `~/.kaizen/projects/<slug>/`.

use anyhow::Result;
use std::path::Path;

pub enum MigrationOutcome {
    Skipped,
    AlreadyMigrated,
    Conflict,
    Migrated,
}

/// Moves `workspace/.kaizen/` → `target/` (one-shot, idempotent).
///
/// - Old absent → `Skipped`
/// - Old has only `MIGRATED.txt` → `AlreadyMigrated`
/// - Both non-empty → `Conflict` (warn, don't merge)
/// - Otherwise: move all entries, write `MIGRATED.txt`, return `Migrated`
pub fn migrate_legacy_in_repo(workspace: &Path, target: &Path) -> Result<MigrationOutcome> {
    let old = workspace.join(".kaizen");
    if !old.exists() {
        return Ok(MigrationOutcome::Skipped);
    }
    let entries: Vec<_> = std::fs::read_dir(&old)?.filter_map(|e| e.ok()).collect();
    if entries.len() == 1 && entries[0].file_name() == "MIGRATED.txt" {
        return Ok(MigrationOutcome::AlreadyMigrated);
    }
    if target.exists() {
        let n = std::fs::read_dir(target)?.filter_map(|e| e.ok()).count();
        if n > 0 {
            return Ok(MigrationOutcome::Conflict);
        }
    }
    std::fs::create_dir_all(target)?;
    for entry in &entries {
        if entry.file_name() == "MIGRATED.txt" {
            continue;
        }
        let src = entry.path();
        let dst = target.join(entry.file_name());
        if std::fs::rename(&src, &dst).is_err() {
            copy_recursive(&src, &dst)?;
            std::fs::remove_dir_all(&src).or_else(|_| std::fs::remove_file(&src))?;
        }
    }
    write_marker(&old, target)?;
    Ok(MigrationOutcome::Migrated)
}

fn write_marker(old: &Path, target: &Path) -> Result<()> {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    std::fs::write(
        old.join("MIGRATED.txt"),
        format!(
            "migrated to: {}\nat: {secs} (unix secs)\nsafe to delete this folder\n",
            target.display()
        ),
    )?;
    Ok(())
}

fn copy_recursive(src: &Path, dst: &Path) -> Result<()> {
    if src.is_dir() {
        std::fs::create_dir_all(dst)?;
        for entry in std::fs::read_dir(src)?.filter_map(|e| e.ok()) {
            copy_recursive(&entry.path(), &dst.join(entry.file_name()))?;
        }
    } else {
        std::fs::copy(src, dst)?;
    }
    Ok(())
}
