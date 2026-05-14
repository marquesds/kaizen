use kaizen::core::event::{Event, EventKind, EventSource, SessionRecord, SessionStatus};
use kaizen::shell::migrate::{cmd_migrate_v1, cmd_migrate_v2};
use kaizen::store::Store;
use serde_json::json;
use std::time::Instant;

const SESSIONS: usize = 100_000;

#[test]
#[ignore = "seeds 100k sessions, migrates v2->v1, checks derived facts"]
fn migrate_v2_round_trip_perf() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let ws = dir.path();
    let ws_key = kaizen::core::paths::canonical(ws)
        .to_string_lossy()
        .to_string();
    unsafe { std::env::set_var("KAIZEN_HOT_LOG", "0") };
    let seed_start = Instant::now();
    let db_path = kaizen::core::workspace::db_path(ws)?;
    let store = Store::open(&db_path)?;
    for n in 0..SESSIONS {
        let id = format!("s{n:06}");
        store.upsert_session(&session(&id, &ws_key))?;
        store.append_event(&event(&id, 0, 1_700_000_000_000 + n as u64))?;
    }
    drop(store);
    let seed_elapsed = seed_start.elapsed();
    unsafe { std::env::remove_var("KAIZEN_HOT_LOG") };
    let v2_start = Instant::now();
    cmd_migrate_v2(Some(ws), false)?;
    let v2_elapsed = v2_start.elapsed();
    let v1_start = Instant::now();
    cmd_migrate_v1(Some(ws))?;
    let v1_elapsed = v1_start.elapsed();
    eprintln!("migrate_v2 perf:");
    eprintln!("  seed: {:.1}s", seed_elapsed.as_secs_f64());
    eprintln!("  v2: {:.1}s", v2_elapsed.as_secs_f64());
    eprintln!("  v1: {:.1}s", v1_elapsed.as_secs_f64());
    let restored = Store::open_read_only(&db_path)?;
    assert_eq!(restored.list_sessions(&ws_key)?.len(), SESSIONS);
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

fn event(session_id: &str, seq: u64, ts_ms: u64) -> Event {
    Event {
        session_id: session_id.into(),
        seq,
        ts_ms,
        ts_exact: true,
        kind: EventKind::ToolCall,
        source: EventSource::Tail,
        tool: Some("bash".into()),
        tool_call_id: Some(format!("{session_id}-{seq}")),
        tokens_in: Some(1),
        tokens_out: Some(1),
        reasoning_tokens: None,
        cost_usd_e6: Some(1),
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
