// SPDX-License-Identifier: AGPL-3.0-or-later
//! Local SQLite cache for provider pull results. Writers use a single transaction per refresh.

use crate::store::Store;
use crate::sync::outbound::OutboundEvent;
use anyhow::Result;
use rusqlite::{Transaction, params};
use std::collections::{HashMap, HashSet};

/// Row `remote_pull_state` (singleton `id = 1`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemotePullState {
    pub query_provider: String,
    pub cursor_json: String,
    pub last_success_ms: Option<i64>,
}

impl Default for RemotePullState {
    fn default() -> Self {
        Self {
            query_provider: "none".to_string(),
            cursor_json: String::new(),
            last_success_ms: None,
        }
    }
}

/// Read/write remote cache tables and pull cursor. Implemented for [`Store`].
pub trait RemoteCacheStore {
    fn get_pull_state(&self) -> Result<RemotePullState>;
    /// Update cursor and last success time (call only after a successful import in the same txn or immediately after commit).
    fn set_pull_state(&self, state: &RemotePullState) -> Result<()>;
    /// Run `f` inside a transaction (use for clear + bulk insert + cursor update).
    fn with_remote_refresh<T>(
        &self,
        f: impl for<'a> FnOnce(&'a Transaction<'_>) -> Result<T>,
    ) -> Result<T>;
}

/// Delete all rows from `remote_*` data tables (use inside `with_remote_refresh` before re-insert).
pub fn clear_remote_cache_tables(tx: &Transaction<'_>) -> Result<()> {
    for table in [
        "remote_sessions",
        "remote_events",
        "remote_tool_spans",
        "remote_repo_snapshots",
        "remote_workspace_facts",
    ] {
        tx.execute(&format!("DELETE FROM {table}"), [])?;
    }
    Ok(())
}

impl RemoteCacheStore for Store {
    fn get_pull_state(&self) -> Result<RemotePullState> {
        let conn = self.conn();
        let row = conn.query_row(
            "SELECT query_provider, cursor_json, last_success_ms FROM remote_pull_state WHERE id = 1",
            [],
            |r| {
                Ok(RemotePullState {
                    query_provider: r.get(0)?,
                    cursor_json: r.get(1)?,
                    last_success_ms: r.get(2)?,
                })
            },
        );
        row.map_err(Into::into)
    }

    fn set_pull_state(&self, state: &RemotePullState) -> Result<()> {
        self.conn().execute(
            "UPDATE remote_pull_state SET query_provider = ?1, cursor_json = ?2, last_success_ms = ?3 WHERE id = 1",
            params![
                &state.query_provider,
                &state.cursor_json,
                state.last_success_ms
            ],
        )?;
        Ok(())
    }

    fn with_remote_refresh<T>(
        &self,
        f: impl for<'a> FnOnce(&'a Transaction<'_>) -> Result<T>,
    ) -> Result<T> {
        let tx = self.conn().unchecked_transaction()?;
        let out = f(&tx)?;
        tx.commit()?;
        Ok(out)
    }
}

impl Store {
    /// Upsert one remote event row (caller runs inside transaction as needed).
    pub fn remote_insert_event(
        &self,
        team_id: &str,
        workspace_hash: &str,
        session_id_hash: &str,
        event_seq: i64,
        json: &str,
    ) -> Result<()> {
        self.conn().execute(
            "INSERT OR REPLACE INTO remote_events (team_id, workspace_hash, session_id_hash, event_seq, json)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![team_id, workspace_hash, session_id_hash, event_seq, json],
        )?;
        Ok(())
    }

    /// JSON payloads in remote_events for this team/workspace (for provider-side retro/merge).
    pub fn list_remote_event_jsons(
        &self,
        team_id: &str,
        workspace_hash: &str,
    ) -> Result<Vec<String>> {
        let mut stmt = self.conn().prepare(
            "SELECT json FROM remote_events WHERE team_id = ?1 AND workspace_hash = ?2 ORDER BY session_id_hash, event_seq",
        )?;
        let rows = stmt.query_map(params![team_id, workspace_hash], |r| r.get::<_, String>(0))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Event-derived aggregates for `summary` / `insights` / `metrics` when `DataSource` is not local.
    pub fn remote_event_aggregate(
        &self,
        team_id: &str,
        workspace_hash: &str,
    ) -> Result<RemoteEventAgg> {
        let mut out = RemoteEventAgg::default();
        let now_ms = now_ms();
        let week_ago = now_ms.saturating_sub(7 * 86_400_000);
        let now_day = now_ms / 86_400_000;

        let mut sessions: HashSet<String> = HashSet::new();
        let mut by_agent: HashMap<String, HashSet<String>> = HashMap::new();
        let mut by_model: HashMap<String, HashSet<String>> = HashMap::new();
        let mut top_tools: HashMap<String, u64> = HashMap::new();
        let mut tool_tokens: HashMap<String, u64> = HashMap::new();
        let mut sessions_by_day: [HashSet<String>; 7] = std::array::from_fn(|_| HashSet::new());
        let mut with_cost: HashSet<String> = HashSet::new();

        for raw in self.list_remote_event_jsons(team_id, workspace_hash)? {
            let o: OutboundEvent = match serde_json::from_str(&raw) {
                Ok(x) => x,
                Err(_) => continue,
            };
            out.event_count = out.event_count.saturating_add(1);
            sessions.insert(o.session_id_hash.clone());
            if o.ts_ms >= week_ago {
                for i in 0..7 {
                    let target = now_day.saturating_sub(6 - i);
                    let d = o.ts_ms / 86_400_000;
                    if d == target {
                        sessions_by_day[i as usize].insert(o.session_id_hash.clone());
                    }
                }
            }
            if let Some(c) = o.cost_usd_e6 {
                out.total_cost_usd_e6 = out.total_cost_usd_e6.saturating_add(c);
                with_cost.insert(o.session_id_hash.clone());
            }
            by_agent
                .entry(o.agent.clone())
                .or_default()
                .insert(o.session_id_hash.clone());
            by_model
                .entry(o.model.clone())
                .or_default()
                .insert(o.session_id_hash.clone());
            if let Some(t) = o.tool.as_ref() {
                *top_tools.entry(t.clone()).or_insert(0) += 1;
                let tok = (o.tokens_in.unwrap_or(0) as u64)
                    .saturating_add(o.tokens_out.unwrap_or(0) as u64)
                    .saturating_add(o.reasoning_tokens.unwrap_or(0) as u64);
                *tool_tokens.entry(t.clone()).or_insert(0) += tok;
            }
        }

        out.session_count = sessions.len() as u64;
        out.sessions_with_cost = with_cost.len() as u64;
        out.by_agent = key_sets_to_top(by_agent);
        out.by_model = key_sets_to_top(by_model);
        out.top_tools = top_hash_to_vec(&top_tools, 10);
        out.tool_token_totals = top_hash_to_vec(&tool_tokens, 20);
        out.sessions_by_day = (0u64..7)
            .map(|i| {
                (
                    day_label(now_day.saturating_sub(6 - i)).to_string(),
                    sessions_by_day[i as usize].len() as u64,
                )
            })
            .collect();
        Ok(out)
    }
}

/// Aggregated remote events for `kaizen summary` / `insights` (and tool rows for `metrics`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RemoteEventAgg {
    pub session_count: u64,
    pub event_count: u64,
    pub total_cost_usd_e6: i64,
    pub sessions_with_cost: u64,
    pub by_agent: Vec<(String, u64)>,
    pub by_model: Vec<(String, u64)>,
    pub top_tools: Vec<(String, u64)>,
    /// Aligned to local `InsightsStats::sessions_by_day` (last 7d, Mon..Sun order in label — same formula as local sessions).
    pub sessions_by_day: Vec<(String, u64)>,
    /// Per-tool total tokens (in+out+reasoning) for merging into `highest_token_tools`.
    pub tool_token_totals: Vec<(String, u64)>,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn day_label(day_idx: u64) -> &'static str {
    ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"][((day_idx + 4) % 7) as usize]
}

fn key_sets_to_top(m: HashMap<String, HashSet<String>>) -> Vec<(String, u64)> {
    let mut v: Vec<(String, u64)> = m.into_iter().map(|(k, s)| (k, s.len() as u64)).collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    v
}

fn top_hash_to_vec(m: &HashMap<String, u64>, limit: usize) -> Vec<(String, u64)> {
    let mut v: Vec<(String, u64)> = m.iter().map(|(a, c)| (a.clone(), *c)).collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    v.truncate(limit);
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Store;
    use tempfile::tempdir;

    #[test]
    fn pull_state_roundtrip() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("t.db");
        let s = Store::open(&db).unwrap();
        let st = s.get_pull_state().unwrap();
        assert_eq!(st.query_provider, "none");
        s.set_pull_state(&RemotePullState {
            query_provider: "posthog".into(),
            cursor_json: r#"{"x":1}"#.into(),
            last_success_ms: Some(42),
        })
        .unwrap();
        let st2 = s.get_pull_state().unwrap();
        assert_eq!(st2.query_provider, "posthog");
        assert_eq!(st2.last_success_ms, Some(42));
    }

    #[test]
    fn clear_remote_tx() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("t.db");
        let s = Store::open(&db).unwrap();
        s.remote_insert_event("t", "w", "s", 0, "{}").unwrap();
        s.with_remote_refresh(|tx| {
            clear_remote_cache_tables(tx)?;
            Ok(())
        })
        .unwrap();
        let n: i64 = s
            .conn()
            .query_row("SELECT COUNT(*) FROM remote_events", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0);
    }
}
