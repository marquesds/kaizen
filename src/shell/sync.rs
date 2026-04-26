// SPDX-License-Identifier: AGPL-3.0-or-later
//! `kaizen sync run` and `kaizen sync status`.

use crate::core::config::{self, try_team_salt};
use crate::shell::cli::workspace_path;
use crate::store::Store;
use crate::sync::FlushExporters;
use crate::sync::flush_outbox_once;
use crate::telemetry;
use anyhow::{Context, Result};
use std::path::Path;
use std::thread;
use std::time::Duration;

/// No stdout (only tracing), matching `kaizen sync run`.
pub fn sync_run_text(workspace: Option<&Path>, once: bool) -> Result<String> {
    let ws = workspace_path(workspace)?;
    let cfg = config::load(&ws)?;
    if cfg.sync.endpoint.is_empty() {
        tracing::info!("sync disabled (sync.endpoint empty)");
        return Ok(String::new());
    }
    let salt = try_team_salt(&cfg.sync)
        .context("sync requires team_salt_hex (64 hex chars), usually in ~/.kaizen/config.toml")?;
    let db_path = ws.join(".kaizen/kaizen.db");
    let store = Store::open(&db_path)?;
    let interval = cfg.sync.flush_interval_ms.max(100);
    let registry = telemetry::load_exporters(&cfg.telemetry, &ws);
    let flush = FlushExporters {
        telemetry: &cfg.telemetry,
        registry: if registry.is_empty() {
            None
        } else {
            Some(&registry)
        },
    };

    loop {
        match flush_outbox_once(&store, &ws, &cfg.sync, &salt, &flush) {
            Ok(stats) => {
                if stats.batches > 0 {
                    tracing::info!(
                        batches = stats.batches,
                        events = stats.events_sent,
                        "sync flush ok"
                    );
                }
            }
            Err(e) => tracing::error!("sync flush failed: {e:#}"),
        }
        if once {
            break;
        }
        thread::sleep(Duration::from_millis(interval));
    }
    Ok(String::new())
}

/// Foreground sync loop; `--once` performs a single flush (tests / cron).
pub fn cmd_sync_run(workspace: Option<&Path>, once: bool) -> Result<()> {
    sync_run_text(workspace, once)?;
    Ok(())
}

/// Same stdout as `kaizen sync status`.
pub fn sync_status_text(workspace: Option<&Path>) -> Result<String> {
    let ws = workspace_path(workspace)?;
    let db_path = ws.join(".kaizen/kaizen.db");
    use std::fmt::Write;
    let mut s = String::new();
    if !db_path.exists() {
        writeln!(&mut s, "no database at {}", db_path.display()).unwrap();
        return Ok(s);
    }
    let store = Store::open(&db_path)?;
    let st = store.sync_status()?;
    writeln!(&mut s, "outbox pending: {}", st.pending_outbox).unwrap();
    match st.last_success_ms {
        Some(ms) => writeln!(&mut s, "last flush ok: {ms} ms since epoch").unwrap(),
        None => writeln!(&mut s, "last flush ok: (never)").unwrap(),
    }
    writeln!(&mut s, "consecutive failures: {}", st.consecutive_failures).unwrap();
    if let Some(e) = &st.last_error {
        writeln!(&mut s, "last error: {e}").unwrap();
    }
    let cfg = config::load(&ws)?;
    if cfg.sync.endpoint.is_empty() {
        writeln!(&mut s, "sync endpoint: (disabled)").unwrap();
    } else {
        writeln!(&mut s, "sync endpoint: {}", cfg.sync.endpoint).unwrap();
    }
    Ok(s)
}

pub fn cmd_sync_status(workspace: Option<&Path>) -> Result<()> {
    print!("{}", sync_status_text(workspace)?);
    Ok(())
}
