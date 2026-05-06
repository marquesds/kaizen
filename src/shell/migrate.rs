// SPDX-License-Identifier: AGPL-3.0-or-later
//! `kaizen migrate`: reversible SQLite ↔ tiered storage bootstrap.

use crate::shell::cli::workspace_path;
use crate::store::{Store, cold_parquet, hot_log::HotLog};
use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::path::Path;

pub fn cmd_migrate_v2(workspace: Option<&Path>, allow_skew: bool) -> Result<()> {
    let ws = workspace_path(workspace)?;
    let root = crate::core::paths::project_data_dir(&ws)?;
    let db_path = root.join("kaizen.db");
    let backup = root.join("kaizen.db.v1.bak");
    if db_path.exists() && !backup.exists() {
        std::fs::copy(&db_path, &backup)
            .with_context(|| format!("backup SQLite DB: {}", backup.display()))?;
    }
    let store = Store::open(&db_path)?;
    let events = workspace_events(&store, &ws.to_string_lossy())?;
    validate_timestamps(&events, allow_skew)?;
    let mut hot = HotLog::open(&root)?;
    for event in &events {
        hot.append(event)?;
    }
    let parts = cold_parquet::write_daily_events(&root, &events)?;
    store.sync_state_set_u64("storage_schema_v", 2)?;
    println!(
        "migrated v2: {} events, {} cold partitions, backup {}",
        events.len(),
        parts.len(),
        backup.display()
    );
    Ok(())
}

pub fn cmd_migrate_v1(workspace: Option<&Path>) -> Result<()> {
    let ws = workspace_path(workspace)?;
    let root = crate::core::paths::project_data_dir(&ws)?;
    let store = Store::open(&root.join("kaizen.db"))?;
    let mut rows = BTreeMap::new();
    for (_, event) in HotLog::replay(&root).unwrap_or_default() {
        rows.insert((event.session_id.clone(), event.seq), event);
    }
    for event in cold_parquet::read_events_dir(&root).unwrap_or_default() {
        rows.insert((event.session_id.clone(), event.seq), event);
    }
    for event in rows.values() {
        store.append_event(event)?;
    }
    store.sync_state_set_u64("storage_schema_v", 1)?;
    println!("migrated v1: {} events restored into SQLite", rows.len());
    Ok(())
}

fn workspace_events(store: &Store, workspace: &str) -> Result<Vec<crate::core::event::Event>> {
    let mut out = Vec::new();
    for session in store.list_sessions(workspace)? {
        out.extend(store.list_events_for_session(&session.id)?);
    }
    out.sort_by(|a, b| (a.ts_ms, &a.session_id, a.seq).cmp(&(b.ts_ms, &b.session_id, b.seq)));
    Ok(out)
}

fn validate_timestamps(events: &[crate::core::event::Event], allow_skew: bool) -> Result<()> {
    if allow_skew {
        return Ok(());
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis() as u64;
    let max = now.saturating_add(86_400_000);
    if let Some(event) = events.iter().find(|e| e.ts_ms > max) {
        anyhow::bail!(
            "future ts_ms {} in session {} seq {} (pass --allow-skew to keep)",
            event.ts_ms,
            event.session_id,
            event.seq
        );
    }
    Ok(())
}
