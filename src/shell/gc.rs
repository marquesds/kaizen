// SPDX-License-Identifier: AGPL-3.0-or-later
//! `kaizen gc` — prune old sessions from the local store.

use crate::core::config;
use crate::shell::cli::workspace_path;
use crate::store::Store;
use anyhow::{Context, Result};
use std::path::Path;

/// Prune sessions older than `keep_days` (from `--days` or `[retention].hot_days`).
pub fn cmd_gc(workspace: Option<&Path>, days: Option<u32>, vacuum: bool) -> Result<()> {
    let ws = workspace_path(workspace)?;
    let cfg = config::load(&ws)?;
    let keep = days.unwrap_or(cfg.retention.hot_days);
    if keep == 0 {
        anyhow::bail!(
            "refusing to prune: keep window is 0 days (unlimited). \
             Set [retention].hot_days > 0 or pass --days <N>"
        );
    }
    let store = Store::open(&ws.join(".kaizen/kaizen.db"))?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let cutoff = now.saturating_sub((keep as u64).saturating_mul(86_400_000));
    let stats = store
        .prune_sessions_started_before(cutoff as i64)
        .context("prune sessions")?;
    println!(
        "pruned {} sessions, {} events (cutoff started_at_ms < {})",
        stats.sessions_removed, stats.events_removed, cutoff
    );
    if vacuum {
        store.vacuum().context("VACUUM")?;
        println!("vacuum complete");
    }
    Ok(())
}
