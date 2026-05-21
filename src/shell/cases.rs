// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::core_loop::{CaseStatus, cases, time};
use crate::shell::cli::workspace_path;
use crate::store::Store;
use anyhow::{Result, anyhow};
use std::path::Path;

pub fn cmd_cases_mine(workspace: Option<&Path>, since: Option<&str>, json: bool) -> Result<()> {
    let store = open(workspace)?;
    let rows = cases::mine(&store, time::parse_window(since, 7)?, time::now_ms())?;
    output(&rows, json)
}

pub fn cmd_cases_create(
    workspace: Option<&Path>,
    session_id: &str,
    reason: &str,
    label: Option<String>,
    json: bool,
) -> Result<()> {
    let store = open(workspace)?;
    let s = store
        .get_session(session_id)?
        .ok_or_else(|| anyhow!("session not found"))?;
    let key = format!("manual:{session_id}:{reason}");
    let row = cases::create_case(&store, &s, &key, reason, label, time::now_ms())?;
    output(&[row], json)
}

pub fn cmd_cases_list(workspace: Option<&Path>, status: Option<String>, json: bool) -> Result<()> {
    output(
        &cases::list(&open(workspace)?, parse_status(status.as_deref()))?,
        json,
    )
}

pub fn cmd_cases_show(workspace: Option<&Path>, id: &str, json: bool) -> Result<()> {
    let store = open(workspace)?;
    let row = cases::get(&store, id)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&row)?);
    } else {
        println!(
            "{} session={} reason={} status={}",
            row.id,
            row.session_id,
            row.reason,
            row.status.as_str()
        );
    }
    Ok(())
}

pub fn cmd_cases_archive(workspace: Option<&Path>, id: &str) -> Result<()> {
    cases::archive(&open(workspace)?, id)?;
    println!("archived case {id}");
    Ok(())
}

fn open(workspace: Option<&Path>) -> Result<Store> {
    let ws = workspace_path(workspace)?;
    Store::open(&crate::core::workspace::db_path(&ws)?)
}

fn output(rows: &[crate::core_loop::CaseRecord], json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(rows)?);
    } else {
        rows.iter().for_each(|r| {
            println!(
                "{} session={} reason={} status={}",
                r.id,
                r.session_id,
                r.reason,
                r.status.as_str()
            )
        });
    }
    Ok(())
}

fn parse_status(raw: Option<&str>) -> Option<CaseStatus> {
    raw.map(|s| {
        if s == "archived" {
            CaseStatus::Archived
        } else {
            CaseStatus::Open
        }
    })
}
