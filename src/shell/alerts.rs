// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::core_loop::{alerts, time};
use crate::shell::cli::workspace_path;
use crate::store::Store;
use anyhow::Result;
use std::path::Path;

pub fn cmd_alerts_check(workspace: Option<&Path>, days: u64, json: bool) -> Result<()> {
    let ws = workspace_path(workspace)?;
    let store = Store::open(&crate::core::workspace::db_path(&ws)?)?;
    let rows = alerts::check_builtin(
        &store,
        &ws.to_string_lossy(),
        time::since_days(days),
        time::now_ms(),
    )?;
    if json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
    } else {
        rows.iter()
            .for_each(|r| println!("{} {} {}", r.severity.as_str(), r.name, r.message));
    }
    Ok(())
}
