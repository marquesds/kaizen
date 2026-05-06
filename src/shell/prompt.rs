// SPDX-License-Identifier: AGPL-3.0-or-later
//! `kaizen prompt` subcommands: list, show, diff.

use crate::prompt;
use crate::store::Store;
use anyhow::{Context, Result};
use serde::Serialize;
use std::path::Path;

#[derive(Serialize)]
struct SnapshotJson {
    fingerprint: String,
    captured_at_ms: u64,
    total_bytes: u64,
    files: Vec<FileJson>,
}

#[derive(Serialize)]
struct FileJson {
    path: String,
    sha256: String,
    bytes: u64,
}

pub fn cmd_prompt_list(workspace: Option<&Path>, json: bool) -> Result<()> {
    let ws = workspace_path(workspace)?;
    let store = open_store(ws.as_path())?;
    let snaps = store.list_prompt_snapshots()?;
    if json {
        println!("{}", serde_json::to_string_pretty(&to_json_list(&snaps))?);
        return Ok(());
    }
    if snaps.is_empty() {
        println!("No prompt snapshots recorded yet.");
        return Ok(());
    }
    for s in &snaps {
        let short = &s.fingerprint[..8.min(s.fingerprint.len())];
        println!(
            "{short}  {}  {} bytes",
            fmt_ts(s.captured_at_ms),
            s.total_bytes
        );
    }
    Ok(())
}

pub fn cmd_prompt_show(fingerprint: &str, workspace: Option<&Path>, json: bool) -> Result<()> {
    let ws = workspace_path(workspace)?;
    let store = open_store(ws.as_path())?;
    let snap = store
        .get_prompt_snapshot(fingerprint)?
        .with_context(|| format!("snapshot not found: {fingerprint}"))?;
    if json {
        println!("{}", serde_json::to_string_pretty(&to_json(&snap))?);
        return Ok(());
    }
    println!("fingerprint: {}", snap.fingerprint);
    println!("captured:    {}", fmt_ts(snap.captured_at_ms));
    println!("total_bytes: {}", snap.total_bytes);
    println!("files:");
    for f in snap.files() {
        println!("  {} ({} bytes)", f.path, f.bytes);
    }
    Ok(())
}

pub fn cmd_prompt_diff(a: &str, b: &str, workspace: Option<&Path>) -> Result<()> {
    let ws = workspace_path(workspace)?;
    let store = open_store(ws.as_path())?;
    let snap_a = store
        .get_prompt_snapshot(a)?
        .with_context(|| format!("snapshot not found: {a}"))?;
    let snap_b = store
        .get_prompt_snapshot(b)?
        .with_context(|| format!("snapshot not found: {b}"))?;
    let diff = prompt::diff::diff(&snap_a, &snap_b);
    if diff.is_empty() {
        println!("No changes between {a} and {b}.");
        return Ok(());
    }
    for p in &diff.added {
        println!("+ {p}");
    }
    for p in &diff.removed {
        println!("- {p}");
    }
    for p in &diff.changed {
        println!("~ {p}");
    }
    Ok(())
}

fn to_json(s: &prompt::PromptSnapshot) -> SnapshotJson {
    SnapshotJson {
        fingerprint: s.fingerprint.clone(),
        captured_at_ms: s.captured_at_ms,
        total_bytes: s.total_bytes,
        files: s
            .files()
            .into_iter()
            .map(|f| FileJson {
                path: f.path,
                sha256: f.sha256,
                bytes: f.bytes,
            })
            .collect(),
    }
}

fn to_json_list(snaps: &[prompt::PromptSnapshot]) -> Vec<SnapshotJson> {
    snaps.iter().map(to_json).collect()
}

fn open_store(ws: &Path) -> Result<Store> {
    let db = crate::core::workspace::db_path(ws)?;
    Store::open(&db).with_context(|| format!("open store: {}", db.display()))
}

fn workspace_path(ws: Option<&Path>) -> Result<std::path::PathBuf> {
    Ok(ws
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::current_dir().expect("cwd")))
}

fn fmt_ts(ms: u64) -> String {
    crate::shell::fmt::fmt_ts(ms)
}
