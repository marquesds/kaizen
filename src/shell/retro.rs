// SPDX-License-Identifier: AGPL-3.0-or-later
//! `kaizen retro` command.

use crate::core::config;
use crate::core::data_source::DataSource;
use crate::metrics::index;
use crate::report::{ReportsDirLock, iso_week_label_utc, to_json, to_markdown, write_atomic};
use crate::retro::types::Report;
use crate::retro::{engine, inputs};
use crate::shell::cli::{maybe_refresh_store, open_workspace_read_store, workspace_path};
use crate::shell::remote_pull::maybe_telemetry_pull;
use crate::store::Store;
use anyhow::Result;
use std::path::{Path, PathBuf};

fn compute_retro(
    workspace: &Path,
    days: u32,
    refresh: bool,
    source: DataSource,
) -> Result<(PathBuf, Report)> {
    let cfg = config::load(workspace)?;
    let db_path = crate::core::workspace::db_path(workspace)?;
    let store = open_workspace_read_store(workspace, refresh || source != DataSource::Local)?;
    let ws_str = workspace.to_string_lossy().to_string();
    maybe_telemetry_pull(workspace, &store, &cfg, source, refresh)?;
    maybe_refresh_store(workspace, &store, refresh)?;
    if refresh
        && let Ok(snapshot) = index::ensure_indexed(&store, workspace, true)
        && let Some(ctx) = crate::sync::ingest_ctx(&cfg, workspace.to_path_buf())
    {
        if let (Ok(facts), Ok(edges)) = (
            store.file_facts_for_snapshot(&snapshot.id),
            store.repo_edges_for_snapshot(&snapshot.id),
        ) {
            let _ =
                crate::sync::smart::enqueue_repo_snapshot(&store, &snapshot, &facts, &edges, &ctx);
        }
        let _ = crate::sync::smart::enqueue_workspace_fact_snapshot(&store, workspace, &ctx);
    }
    let read_store = Store::open_query(&db_path)?;

    let end_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let start_ms = end_ms.saturating_sub((days as u64).saturating_mul(86_400_000));

    let team_id = if cfg.sync.team_id.is_empty() {
        None
    } else {
        Some(cfg.sync.team_id.as_str())
    };
    let workspace_hash = crate::sync::ingest_ctx(&cfg, workspace.to_path_buf())
        .as_ref()
        .and_then(crate::sync::smart::workspace_hash_for);
    let inputs = inputs::load_inputs_for_data_source(
        &read_store,
        workspace,
        &ws_str,
        start_ms,
        end_ms,
        source,
        team_id,
        workspace_hash.as_deref(),
    )?;
    let reports_dir = crate::core::paths::project_data_dir(workspace)?.join("reports");
    let week_label = iso_week_label_utc();
    let prior = inputs::prior_bet_fingerprints(&reports_dir)?;
    let mut report = engine::run(&inputs, &prior);
    report.meta.week_label = week_label;
    Ok((reports_dir, report))
}

/// Build retro report (shared by CLI and MCP; no report file I/O).
pub fn run_retro_report(
    workspace: Option<&Path>,
    days: u32,
    refresh: bool,
    source: DataSource,
) -> Result<Report> {
    let ws = workspace_path(workspace)?;
    let (_reports_dir, report) = compute_retro(&ws, days, refresh, source)?;
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
    source: DataSource,
) -> Result<String> {
    let ws = workspace_path(workspace)?;
    let (reports_dir, report) = compute_retro(&ws, days, refresh, source)?;
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
    source: DataSource,
) -> Result<()> {
    print!(
        "{}",
        retro_stdout(workspace, days, dry_run, json_out, force, refresh, source)?
    );
    Ok(())
}
