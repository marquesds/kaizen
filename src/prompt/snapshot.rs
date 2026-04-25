// SPDX-License-Identifier: AGPL-3.0-or-later
//! Capture a `PromptSnapshot` by scanning prompt-relevant files in a workspace.

use crate::prompt::types::{PromptFile, PromptSnapshot};
use anyhow::Result;
use blake3::Hasher;
use std::fs;
use std::path::Path;

static FIXED_FILES: &[&str] = &["CLAUDE.md", "AGENTS.md", "CONTRIBUTING.md"];

/// Capture a snapshot of all prompt-relevant files under `workspace`.
pub fn capture(workspace: &Path, now_ms: u64) -> Result<PromptSnapshot> {
    let mut files = collect_files(workspace)?;
    files.sort_by(|a, b| a.path.cmp(&b.path));
    let total_bytes = files.iter().map(|f| f.bytes).sum();
    let fingerprint = fingerprint_files(&files);
    let files_json = serde_json::to_string(&files)?;
    Ok(PromptSnapshot {
        fingerprint,
        captured_at_ms: now_ms,
        files_json,
        total_bytes,
    })
}

/// Blake3 fingerprint over sorted file path+hash pairs.
pub fn fingerprint_files(files: &[PromptFile]) -> String {
    let mut hasher = Hasher::new();
    for f in files {
        hasher.update(f.path.as_bytes());
        hasher.update(b":");
        hasher.update(f.sha256.as_bytes());
        hasher.update(b"\n");
    }
    hex::encode(hasher.finalize().as_bytes())
}

fn collect_files(workspace: &Path) -> Result<Vec<PromptFile>> {
    let mut out = Vec::new();
    for name in FIXED_FILES {
        let p = workspace.join(name);
        if p.is_file() {
            out.push(read_prompt_file(workspace, &p)?);
        }
    }
    out.extend(scan_ext(workspace, ".cursor/rules", "mdc")?);
    out.extend(scan_skill_mds(workspace)?);
    Ok(out)
}

fn scan_ext(workspace: &Path, dir: &str, ext: &str) -> Result<Vec<PromptFile>> {
    let dir_path = workspace.join(dir);
    if !dir_path.is_dir() {
        return Ok(vec![]);
    }
    let mut out = Vec::new();
    for entry in fs::read_dir(&dir_path)? {
        let entry = entry?;
        let p = entry.path();
        if p.is_file() && p.extension().and_then(|x| x.to_str()) == Some(ext) {
            out.push(read_prompt_file(workspace, &p)?);
        }
    }
    Ok(out)
}

fn scan_skill_mds(workspace: &Path) -> Result<Vec<PromptFile>> {
    let skills_dir = workspace.join(".cursor/skills");
    if !skills_dir.is_dir() {
        return Ok(vec![]);
    }
    let mut out = Vec::new();
    for entry in fs::read_dir(&skills_dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            let skill_md = entry.path().join("SKILL.md");
            if skill_md.is_file() {
                out.push(read_prompt_file(workspace, &skill_md)?);
            }
        }
    }
    Ok(out)
}

fn read_prompt_file(workspace: &Path, abs: &Path) -> Result<PromptFile> {
    let bytes = fs::read(abs)?;
    let sha256 = hex::encode(blake3::hash(&bytes).as_bytes());
    let rel = abs
        .strip_prefix(workspace)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| abs.to_string_lossy().into_owned());
    Ok(PromptFile {
        path: rel,
        sha256,
        bytes: bytes.len() as u64,
    })
}
