// SPDX-License-Identifier: AGPL-3.0-or-later
//! Periodic transcript scanner owned by the daemon runtime.

use crate::store::Store;
use anyhow::Result;
use std::path::{Path, PathBuf};

pub(super) async fn scanner_loop(ws: PathBuf) {
    loop {
        scan_once(ws.clone()).await;
        tokio::time::sleep(scan_interval(&ws)).await;
    }
}

async fn scan_once(ws: PathBuf) {
    if let Err(err) = tokio::task::spawn_blocking(move || scan_workspace(&ws)).await {
        tracing::warn!(%err, "daemon scanner task join failed");
    }
}

fn scan_workspace(ws: &Path) -> Result<()> {
    let cfg = crate::core::config::load(ws)?;
    let store = Store::open(&crate::core::workspace::db_path(ws)?)?;
    let ws_str = ws.to_string_lossy().to_string();
    crate::shell::cli::maybe_scan_all_agents(ws, &cfg, &ws_str, &store, true)?;
    crate::shell::cli::maybe_auto_prune_after_scan(&store, &cfg)
}

fn scan_interval(ws: &Path) -> std::time::Duration {
    let secs = crate::core::config::load(ws)
        .map(|cfg| cfg.scan.min_rescan_seconds.max(5))
        .unwrap_or(300);
    std::time::Duration::from_secs(secs)
}
