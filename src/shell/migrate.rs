// SPDX-License-Identifier: AGPL-3.0-or-later
//! `kaizen migrate`: reversible SQLite ↔ tiered storage bootstrap.

use crate::shell::cli::workspace_path;
use crate::store::SessionFilter;
use crate::store::{Store, cold_parquet, hot_log::HotLog};
use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::path::Path;

const MIGRATE_SESSION_PAGE: usize = 512;
const MIGRATE_PARQUET_CHUNK: usize = 8192;

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
    let workspace = ws.to_string_lossy().to_string();
    validate_timestamps(&store, &workspace, allow_skew)?;
    let mut hot = HotLog::open(&root)?;
    let mut cold = cold_parquet::DailyEventWriter::new(&root, MIGRATE_PARQUET_CHUNK);
    let mut total = 0_usize;
    for_workspace_events(&store, &workspace, |event| {
        hot.append(event)?;
        cold.push(event.clone())?;
        total += 1;
        Ok(())
    })?;
    hot.flush()?;
    let parts = cold.finish()?;
    store.sync_state_set_u64("storage_schema_v", 2)?;
    println!(
        "migrated v2: {} events, {} cold partitions, backup {}",
        total,
        parts.len(),
        backup.display()
    );
    Ok(())
}

fn for_workspace_events<F>(store: &Store, workspace: &str, mut f: F) -> Result<()>
where
    F: FnMut(&crate::core::event::Event) -> Result<()>,
{
    let mut offset = 0;
    loop {
        let page = store.list_sessions_page(
            workspace,
            offset,
            MIGRATE_SESSION_PAGE,
            SessionFilter::default(),
        )?;
        for session in page.rows {
            for event in store.list_events_for_session(&session.id)? {
                f(&event)?;
            }
        }
        let Some(next) = page.next_offset else {
            break;
        };
        offset = next;
    }
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

fn validate_timestamps(store: &Store, workspace: &str, allow_skew: bool) -> Result<()> {
    if allow_skew {
        return Ok(());
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis() as u64;
    let max = now.saturating_add(86_400_000);
    for_workspace_events(store, workspace, |event| {
        if event.ts_ms <= max {
            return Ok(());
        }
        anyhow::bail!(
            "future ts_ms {} in session {} seq {} (pass --allow-skew to keep)",
            event.ts_ms,
            event.session_id,
            event.seq
        )
    })?;
    Ok(())
}
