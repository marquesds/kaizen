//! `kaizen sync run` and `kaizen sync status`.

use crate::core::config::{self, try_team_salt};
use crate::shell::cli::workspace_path;
use crate::store::Store;
use crate::sync::flush_outbox_once;
use anyhow::{Context, Result};
use std::path::Path;
use std::thread;
use std::time::Duration;

/// Foreground sync loop; `--once` performs a single flush (tests / cron).
pub fn cmd_sync_run(workspace: Option<&Path>, once: bool) -> Result<()> {
    let ws = workspace_path(workspace)?;
    let cfg = config::load(&ws)?;
    if cfg.sync.endpoint.is_empty() {
        tracing::info!("sync disabled (sync.endpoint empty)");
        return Ok(());
    }
    let salt = try_team_salt(&cfg.sync).context(
        "sync requires team_salt_hex (64 hex chars), usually in ~/.kaizen/config.toml",
    )?;
    let db_path = ws.join(".kaizen/kaizen.db");
    let store = Store::open(&db_path)?;
    let interval = cfg.sync.flush_interval_ms.max(100);

    loop {
        match flush_outbox_once(&store, &ws, &cfg.sync, &salt) {
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
    Ok(())
}

pub fn cmd_sync_status(workspace: Option<&Path>) -> Result<()> {
    let ws = workspace_path(workspace)?;
    let db_path = ws.join(".kaizen/kaizen.db");
    if !db_path.exists() {
        println!("no database at {}", db_path.display());
        return Ok(());
    }
    let store = Store::open(&db_path)?;
    let st = store.sync_status()?;
    println!("outbox pending: {}", st.pending_outbox);
    match st.last_success_ms {
        Some(ms) => println!("last flush ok: {ms} ms since epoch"),
        None => println!("last flush ok: (never)"),
    }
    println!("consecutive failures: {}", st.consecutive_failures);
    match &st.last_error {
        Some(e) => println!("last error: {e}"),
        None => {}
    }
    let cfg = config::load(&ws)?;
    if cfg.sync.endpoint.is_empty() {
        println!("sync endpoint: (disabled)");
    } else {
        println!("sync endpoint: {}", cfg.sync.endpoint);
    }
    Ok(())
}
