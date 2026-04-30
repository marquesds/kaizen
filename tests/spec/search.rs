// SPDX-License-Identifier: AGPL-3.0-or-later
use kaizen::core::event::{Event, EventKind, EventSource, SessionRecord, SessionStatus};
use kaizen::search::{SearchQuery, reindex_workspace, search};
use kaizen::shell::search::sessions_search_text;
use kaizen::store::Store;
use quint_connect::*;
use serde_json::json;
use tempfile::TempDir;

struct SearchDriver;

impl Driver for SearchDriver {
    type State = ();

    fn step(&mut self, _step: &Step) -> Result {
        Ok(())
    }
}

#[quint_run(spec = "specs/search.qnt", max_samples = 10, max_steps = 8)]
fn search_run() -> impl Driver {
    SearchDriver
}

#[test]
fn reindex_and_query_round_trip() -> anyhow::Result<()> {
    let dir = TempDir::new()?;
    let ws = dir.path();
    std::fs::create_dir_all(ws.join(".kaizen"))?;
    let store = Store::open(&ws.join(".kaizen/kaizen.db"))?;
    let session = session(ws);
    store.upsert_session(&session)?;
    let event = event("deadlock inside scheduler", 6_000);
    store.append_event(&event)?;
    drop(store);
    let store = Store::open(&ws.join(".kaizen/kaizen.db"))?;
    let cfg = kaizen::core::config::Config::default();
    let stats = reindex_workspace(
        &ws.join(".kaizen"),
        ws,
        std::slice::from_ref(&session),
        vec![(session.clone(), event)],
        &cfg,
    )?;
    assert_eq!(stats.docs_indexed, 1);
    let opts = SearchQuery {
        query: "deadlock AND tokens_total:>5000".into(),
        since_ms: None,
        agent: Some("claude-code".into()),
        kind: Some("message".into()),
        limit: 10,
    };
    let hits = search(&ws.join(".kaizen"), &opts, ws, &[0; 32], |s, q| {
        store.get_event(s, q)
    })?;
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].session_id, "s1");
    Ok(())
}

#[test]
fn missing_index_errors_with_reindex_hint() -> anyhow::Result<()> {
    let dir = TempDir::new()?;
    let ws = dir.path();
    let store = Store::open(&ws.join(".kaizen/kaizen.db"))?;
    let session = session(ws);
    store.upsert_session(&session)?;
    store.append_event(&event("deadlock inside scheduler", 6_000))?;
    drop(store);
    let _ = std::fs::remove_dir_all(ws.join(".kaizen/search"));

    let err = sessions_search_text(Some(ws), "deadlock", None, None, None, 10)
        .expect_err("missing index should not fall back to DB scan");
    assert!(err.to_string().contains("kaizen search reindex"));
    Ok(())
}

fn session(ws: &std::path::Path) -> SessionRecord {
    SessionRecord {
        id: "s1".into(),
        agent: "claude-code".into(),
        model: None,
        workspace: ws.to_string_lossy().to_string(),
        started_at_ms: 1,
        ended_at_ms: None,
        status: SessionStatus::Running,
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

fn event(text: &str, tokens: u32) -> Event {
    Event {
        session_id: "s1".into(),
        seq: 0,
        ts_ms: 1,
        ts_exact: true,
        kind: EventKind::Message,
        source: EventSource::Tail,
        tool: None,
        tool_call_id: None,
        tokens_in: Some(tokens),
        tokens_out: None,
        reasoning_tokens: None,
        cost_usd_e6: None,
        stop_reason: None,
        latency_ms: None,
        ttft_ms: None,
        retry_count: None,
        context_used_tokens: None,
        context_max_tokens: None,
        cache_creation_tokens: None,
        cache_read_tokens: None,
        system_prompt_tokens: None,
        payload: json!({ "text": text }),
    }
}
