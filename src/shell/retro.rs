//! `kaizen retro` command.

use crate::core::config;
use crate::report::{ReportsDirLock, iso_week_label_utc, to_json, to_markdown, write_atomic};
use crate::retro::{engine, inputs};
use crate::shell::cli::{scan_all_agents, workspace_path};
use crate::store::Store;
use anyhow::Result;
use std::path::Path;

/// Run retro for the last `days` days.
pub fn cmd_retro(
    workspace: Option<&Path>,
    days: u32,
    dry_run: bool,
    json_out: bool,
    force: bool,
) -> Result<()> {
    let ws = workspace_path(workspace)?;
    let cfg = config::load(&ws)?;
    let db_path = ws.join(".kaizen/kaizen.db");
    let store = Store::open(&db_path)?;
    let ws_str = ws.to_string_lossy().to_string();
    scan_all_agents(&ws, &cfg, &ws_str, &store)?;

    let end_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let start_ms = end_ms.saturating_sub((days as u64).saturating_mul(86_400_000));

    let inputs = inputs::load_inputs(&store, &ws, &ws_str, start_ms, end_ms)?;
    let reports_dir = ws.join(".kaizen/reports");
    let week_label = iso_week_label_utc();
    let out_path = reports_dir.join(format!("{week_label}.md"));

    if !force && !dry_run && !json_out && out_path.exists() {
        println!(
            "retro: {} already exists (use --force to overwrite)",
            out_path.display()
        );
        return Ok(());
    }

    let prior = inputs::prior_bet_fingerprints(&reports_dir)?;
    let mut report = engine::run(&inputs, &prior);
    report.meta.week_label = week_label.clone();

    if json_out {
        println!("{}", to_json(&report)?);
        return Ok(());
    }

    let md = to_markdown(&report);
    if dry_run {
        print!("{md}");
        return Ok(());
    }

    let _lock = ReportsDirLock::acquire(&reports_dir)?;
    write_atomic(&out_path, md.as_bytes())?;
    println!("wrote {}", out_path.display());
    Ok(())
}
