// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::core_loop::time;
use crate::shell::cli::workspace_path;
use crate::store::Store;
use anyhow::Result;
use std::path::Path;

pub fn cmd_query(
    workspace: Option<&Path>,
    expr: &str,
    since: Option<&str>,
    limit: usize,
    json: bool,
) -> Result<()> {
    let ws = workspace_path(workspace)?;
    let store = Store::open(&crate::core::workspace::db_path(&ws)?)?;
    let start = time::parse_window(since, 7)?;
    let hits = crate::core_loop::query::run(&store, &ws.to_string_lossy(), expr, start, limit)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&hits)?);
    } else {
        print_hits(&hits);
    }
    Ok(())
}

pub fn print_hits(hits: &[crate::core_loop::TraceHit]) {
    println!("{:<40} {:>6} {:<12} SUMMARY", "SESSION", "SEQ", "KIND");
    for h in hits {
        let seq = h.seq.map(|s| s.to_string()).unwrap_or_else(|| "-".into());
        println!(
            "{:<40} {:>6} {:<12} {}",
            h.session_id, seq, h.kind, h.summary
        );
    }
}
