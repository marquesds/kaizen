// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::core_loop::{AlertEvent, AlertSeverity};
use crate::store::Store;
use anyhow::Result;
use rusqlite::params;

pub fn cost_spike(
    store: &Store,
    workspace: &str,
    start_ms: u64,
    now_ms: u64,
) -> Result<Vec<AlertEvent>> {
    let stats = store.summary_stats(workspace)?;
    if stats.session_count == 0 || stats.total_cost_usd_e6 <= 0 {
        return Ok(vec![]);
    }
    let avg = stats.total_cost_usd_e6 / stats.session_count as i64;
    store
        .list_sessions(workspace)?
        .into_iter()
        .filter(|s| s.started_at_ms >= start_ms)
        .filter_map(|s| session_cost(store, &s.id).ok().map(|c| (s, c)))
        .filter(|(_, c)| *c > avg * 4)
        .map(|(s, c)| emit_cost(store, &s.id, c, start_ms, now_ms))
        .collect()
}

fn emit_cost(
    store: &Store,
    session_id: &str,
    cost: i64,
    start_ms: u64,
    now_ms: u64,
) -> Result<AlertEvent> {
    crate::core_loop::alerts::emit(
        store,
        &format!("builtin:cost_spike:{session_id}:{start_ms}"),
        "cost_spike",
        AlertSeverity::Warning,
        &format!(
            "session cost ${:.4} exceeds 4x average",
            cost as f64 / 1_000_000.0
        ),
        Some(session_id),
        now_ms,
    )
}

fn session_cost(store: &Store, id: &str) -> Result<i64> {
    store
        .conn()
        .query_row(
            "SELECT COALESCE(SUM(cost_usd_e6), 0) FROM events WHERE session_id = ?1",
            params![id],
            |r| r.get(0),
        )
        .map_err(Into::into)
}
