// SPDX-License-Identifier: AGPL-3.0-or-later
#![cfg(feature = "analytics-duckdb")]

use kaizen::core::event::{Event, EventKind, EventSource, SessionRecord, SessionStatus};
use kaizen::store::{Store, cold_parquet, query::QueryStore};
use serde_json::json;

#[test]
fn summary_stats_include_cold_parquet_cost() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let ws = dir.path().to_string_lossy().to_string();
    let store = Store::open(&dir.path().join("kaizen.db"))?;
    store.upsert_session(&session("s1", &ws))?;
    cold_parquet::write_daily_events(dir.path(), &[event("s1")])?;
    let query = QueryStore::open(dir.path())?;
    let stats = query.summary_stats(&store, &ws)?;
    assert_eq!(stats.session_count, 1);
    assert_eq!(stats.total_cost_usd_e6, 7);
    assert_eq!(query.cold_event_count()?, 1);
    Ok(())
}

fn session(id: &str, workspace: &str) -> SessionRecord {
    SessionRecord {
        id: id.into(),
        agent: "codex".into(),
        model: Some("gpt".into()),
        workspace: workspace.into(),
        started_at_ms: 1_700_000_000_000,
        ended_at_ms: Some(1_700_000_001_000),
        status: SessionStatus::Done,
        trace_path: String::new(),
        start_commit: None,
        end_commit: None,
        branch: None,
        dirty_start: None,
        dirty_end: None,
        repo_binding_source: None,
        prompt_fingerprint: None,
        parent_session_id: None,
        agent_version: None,
        os: None,
        arch: None,
        repo_file_count: None,
        repo_total_loc: None,
    }
}

fn event(session_id: &str) -> Event {
    Event {
        session_id: session_id.into(),
        seq: 0,
        ts_ms: 1_700_000_000_000,
        ts_exact: true,
        kind: EventKind::ToolCall,
        source: EventSource::Tail,
        tool: Some("bash".into()),
        tool_call_id: Some("call-1".into()),
        tokens_in: Some(1),
        tokens_out: Some(1),
        reasoning_tokens: None,
        cost_usd_e6: Some(7),
        stop_reason: None,
        latency_ms: None,
        ttft_ms: None,
        retry_count: None,
        context_used_tokens: None,
        context_max_tokens: None,
        cache_creation_tokens: None,
        cache_read_tokens: None,
        system_prompt_tokens: None,
        payload: json!({"path": "src/main.rs"}),
    }
}
