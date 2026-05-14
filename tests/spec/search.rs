// SPDX-License-Identifier: AGPL-3.0-or-later
use kaizen::core::event::{Event, EventKind, EventSource, SessionRecord, SessionStatus};
use kaizen::search::{SearchQuery, reindex_workspace, search};
use kaizen::shell::search::sessions_search_text;
use kaizen::store::Store;
use quint_connect::*;
use serde_json::json;
use std::sync::{Mutex, OnceLock};
use tempfile::TempDir;

fn env_lock() -> &'static Mutex<()> {
    static L: OnceLock<Mutex<()>> = OnceLock::new();
    L.get_or_init(|| Mutex::new(()))
}

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
    let _guard = env_lock().lock().unwrap();
    let home = TempDir::new()?;
    let dir = TempDir::new()?;
    let ws = dir.path();
    unsafe { std::env::set_var("KAIZEN_HOME", home.path()) };
    let data_dir = kaizen::core::paths::project_data_dir(ws)?;
    let store = Store::open(&data_dir.join("kaizen.db"))?;
    let session = session(ws);
    store.upsert_session(&session)?;
    let event = event("deadlock inside scheduler", 6_000);
    store.append_event(&event)?;
    drop(store);
    let store = Store::open(&data_dir.join("kaizen.db"))?;
    let cfg = kaizen::core::config::Config::default();
    let stats = reindex_workspace(
        &data_dir,
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
    let hits = search(&data_dir, &opts, ws, &[0; 32], |s, q| store.get_event(s, q))?;
    unsafe { std::env::remove_var("KAIZEN_HOME") };
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].session_id, "s1");
    Ok(())
}

#[test]
fn missing_index_errors_with_reindex_hint() -> anyhow::Result<()> {
    let _guard = env_lock().lock().unwrap();
    let home = TempDir::new()?;
    let dir = TempDir::new()?;
    let ws = dir.path();
    unsafe { std::env::set_var("KAIZEN_HOME", home.path()) };
    let data_dir = kaizen::core::paths::project_data_dir(ws)?;
    let store = Store::open(&data_dir.join("kaizen.db"))?;
    let session = session(ws);
    store.upsert_session(&session)?;
    store.append_event(&event("deadlock inside scheduler", 6_000))?;
    drop(store);
    let _ = std::fs::remove_dir_all(data_dir.join("search"));
    let err = sessions_search_text(Some(ws), "deadlock", None, None, None, 10)
        .expect_err("missing index should not fall back to DB scan");
    unsafe { std::env::remove_var("KAIZEN_HOME") };
    assert!(err.to_string().contains("kaizen search reindex"));
    Ok(())
}

#[test]
fn exact_tool_search_uses_bounded_sql_path() -> anyhow::Result<()> {
    let _guard = env_lock().lock().unwrap();
    let home = TempDir::new()?;
    let dir = TempDir::new()?;
    let ws = dir.path();
    unsafe { std::env::set_var("KAIZEN_HOME", home.path()) };
    let data_dir = kaizen::core::paths::project_data_dir(ws)?;
    let store = Store::open(&data_dir.join("kaizen.db"))?;
    store.upsert_session(&session(ws))?;
    store.append_event(&tool_event("read_file"))?;
    let _ = std::fs::remove_dir_all(data_dir.join("search"));
    let out = sessions_search_text(Some(ws), "read_file", None, None, None, 10)?;
    unsafe { std::env::remove_var("KAIZEN_HOME") };
    assert!(out.contains("read_file"));
    assert!(!out.contains("search index unavailable"));
    Ok(())
}

fn session(ws: &std::path::Path) -> SessionRecord {
    SessionRecord {
        id: "s1".into(),
        agent: "claude-code".into(),
        model: None,
        workspace: kaizen::core::workspace::canonical(ws)
            .to_string_lossy()
            .to_string(),
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

fn tool_event(tool: &str) -> Event {
    Event {
        kind: EventKind::ToolCall,
        tool: Some(tool.into()),
        payload: json!({ "path": "src/main.rs" }),
        ..event(tool, 10)
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
