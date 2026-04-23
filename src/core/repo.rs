// SPDX-License-Identifier: AGPL-3.0-or-later
//! Git-bound repo facts. Shell out only at boundary.

use anyhow::{Context, Result};
use blake3::Hasher;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Default)]
pub struct RepoBinding {
    pub start_commit: Option<String>,
    pub end_commit: Option<String>,
    pub branch: Option<String>,
    pub dirty_start: Option<bool>,
    pub dirty_end: Option<bool>,
    pub source: Option<String>,
}

pub fn binding_for_session(
    workspace: &Path,
    started_at_ms: u64,
    ended_at_ms: Option<u64>,
) -> RepoBinding {
    if !workspace.join(".git").exists() {
        return RepoBinding::default();
    }
    let branch = git_trimmed(workspace, &["rev-parse", "--abbrev-ref", "HEAD"]).ok();
    let dirty = git_dirty(workspace).ok();
    let start_commit = git_commit_before(workspace, started_at_ms).ok().flatten();
    let end_commit = git_commit_before(workspace, ended_at_ms.unwrap_or(started_at_ms))
        .ok()
        .flatten()
        .or_else(|| start_commit.clone());
    RepoBinding {
        start_commit,
        end_commit,
        branch,
        dirty_start: dirty,
        dirty_end: dirty,
        source: Some("git_shell".into()),
    }
}

pub fn repo_head(workspace: &Path) -> Result<Option<String>> {
    if !workspace.join(".git").exists() {
        return Ok(None);
    }
    git_trimmed(workspace, &["rev-parse", "HEAD"]).map(Some)
}

pub fn dirty_fingerprint(workspace: &Path) -> Result<String> {
    let status = git_output(workspace, &["status", "--porcelain"])?;
    let mut hasher = Hasher::new();
    hasher.update(status.as_bytes());
    Ok(hex::encode(hasher.finalize().as_bytes()))
}

pub fn tracked_files(workspace: &Path) -> Result<Vec<String>> {
    let out = git_output(workspace, &["ls-files"])?;
    Ok(out
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
        .collect())
}

fn git_commit_before(workspace: &Path, ts_ms: u64) -> Result<Option<String>> {
    let secs = (ts_ms / 1000).max(1);
    let out = git_output(
        workspace,
        &["rev-list", "-1", &format!("--before=@{secs}"), "HEAD"],
    )?;
    let trimmed = out.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    Ok(Some(trimmed.to_string()))
}

fn git_dirty(workspace: &Path) -> Result<bool> {
    let out = git_output(workspace, &["status", "--porcelain"])?;
    Ok(!out.trim().is_empty())
}

fn git_trimmed(workspace: &Path, args: &[&str]) -> Result<String> {
    Ok(git_output(workspace, args)?.trim().to_string())
}

fn git_output(workspace: &Path, args: &[&str]) -> Result<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(workspace)
        .args(args)
        .output()
        .with_context(|| format!("git {:?}", args))?;
    if !out.status.success() {
        anyhow::bail!(
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}
