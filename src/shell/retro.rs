// SPDX-License-Identifier: AGPL-3.0-or-later
//! `kaizen retro` command.

use crate::core::config;
use crate::metrics::index;
use crate::report::{ReportsDirLock, iso_week_label_utc, to_json, to_markdown, write_atomic};
use crate::retro::types::Report;
use crate::retro::{engine, inputs};
use crate::shell::cli::{maybe_scan_all_agents, workspace_path};
use crate::store::Store;
use anyhow::Result;
use std::path::{Path, PathBuf};

fn compute_retro(workspace: &Path, days: u32, refresh: bool) -> Result<(PathBuf, Report)> {
    let cfg = config::load(workspace)?;
    let db_path = workspace.join(".kaizen/kaizen.db");
    let store = Store::open(&db_path)?;
    let ws_str = workspace.to_string_lossy().to_string();
    maybe_scan_all_agents(workspace, &cfg, &ws_str, &store, refresh)?;
    if let Ok(snapshot) = index::ensure_indexed(&store, workspace, false)
        && let Some(ctx) = crate::sync::ingest_ctx(&cfg, workspace.to_path_buf())
        && let Ok(facts) = store.file_facts_for_snapshot(&snapshot.id)
        && let Ok(edges) = store.repo_edges_for_snapshot(&snapshot.id)
    {
        let _ = crate::sync::smart::enqueue_repo_snapshot(&store, &snapshot, &facts, &edges, &ctx);
    }

    let end_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let start_ms = end_ms.saturating_sub((days as u64).saturating_mul(86_400_000));

    let inputs = inputs::load_inputs(&store, workspace, &ws_str, start_ms, end_ms)?;
    let reports_dir = workspace.join(".kaizen/reports");
    let week_label = iso_week_label_utc();
    let prior = inputs::prior_bet_fingerprints(&reports_dir)?;
    let mut report = engine::run(&inputs, &prior);
    report.meta.week_label = week_label;
    Ok((reports_dir, report))
}

/// Build retro report (shared by CLI and MCP; no report file I/O).
pub fn run_retro_report(workspace: Option<&Path>, days: u32, refresh: bool) -> Result<Report> {
    let ws = workspace_path(workspace)?;
    let (_reports_dir, report) = compute_retro(&ws, days, refresh)?;
    Ok(report)
}

/// Text that would be printed to stdout (exact CLI parity with `kaizen retro`).
pub fn retro_stdout(
    workspace: Option<&Path>,
    days: u32,
    dry_run: bool,
    json_out: bool,
    force: bool,
    refresh: bool,
) -> Result<String> {
    let ws = workspace_path(workspace)?;
    let (reports_dir, report) = compute_retro(&ws, days, refresh)?;
    let week_label = report.meta.week_label.clone();
    let out_path = reports_dir.join(format!("{week_label}.md"));

    if !force && !dry_run && !json_out && out_path.exists() {
        return Ok(format!(
            "retro: {} already exists (use --force to overwrite)\n",
            out_path.display()
        ));
    }

    if json_out {
        return to_json(&report);
    }

    let md = to_markdown(&report);
    if dry_run {
        return Ok(md);
    }

    let _lock = ReportsDirLock::acquire(&reports_dir)?;
    write_atomic(&out_path, md.as_bytes())?;
    Ok(format!("wrote {}\n", out_path.display()))
}

/// Run retro for the last `days` days.
pub fn cmd_retro(
    workspace: Option<&Path>,
    days: u32,
    dry_run: bool,
    json_out: bool,
    force: bool,
    refresh: bool,
) -> Result<()> {
    print!(
        "{}",
        retro_stdout(workspace, days, dry_run, json_out, force, refresh)?
    );
    Ok(())
}
