// SPDX-License-Identifier: AGPL-3.0-or-later
//! Commands for interchange and audit extensions.

use crate::extensions::{aggregates, atif, hash_chain, jsonl};
use crate::shell::cli::workspace_path;
use crate::store::Store;
use anyhow::Result;
use std::path::Path;

pub fn cmd_aggregates_rebuild(workspace: Option<&Path>) -> Result<()> {
    let ws = workspace_path(workspace)?;
    let store = open_store(&ws)?;
    let count = aggregates::rebuild_workspace(&store, &ws.to_string_lossy())?;
    println!("rebuilt session_aggregates: {count}");
    Ok(())
}

pub fn cmd_export_atif(workspace: Option<&Path>, session: &str) -> Result<()> {
    let ws = workspace_path(workspace)?;
    let store = open_store(&ws)?;
    let doc = atif::export_session(&store, session)?;
    println!("{}", serde_json::to_string_pretty(&doc)?);
    Ok(())
}

pub fn cmd_import_atif(workspace: Option<&Path>, file: &Path) -> Result<()> {
    let ws = workspace_path(workspace)?;
    let store = open_store(&ws)?;
    let doc = atif::import_file(&store, file, &ws.to_string_lossy())?;
    println!(
        "imported atif: {} steps={}",
        doc.trajectory_id,
        doc.steps.len()
    );
    Ok(())
}

pub fn cmd_import_jsonl(workspace: Option<&Path>, file: &Path) -> Result<()> {
    let ws = workspace_path(workspace)?;
    let store = open_store(&ws)?;
    let report = jsonl::import_file(&store, file, &ws.to_string_lossy())?;
    println!(
        "imported jsonl: events={} sessions_created={}",
        report.imported_events, report.sessions_created
    );
    Ok(())
}

pub fn cmd_verify_hash_chain(workspace: Option<&Path>, session: Option<&str>) -> Result<()> {
    let ws = workspace_path(workspace)?;
    let store = open_store(&ws)?;
    let report = hash_chain::verify(&store, &ws.to_string_lossy(), session)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn open_store(workspace: &Path) -> Result<Store> {
    Store::open(&crate::core::workspace::db_path(workspace)?)
}
