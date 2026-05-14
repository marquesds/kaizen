use kaizen::core::event::{Event, EventKind, EventSource, SessionRecord, SessionStatus};
use kaizen::metrics::types::{FileFact, RepoSnapshotRecord};
use serde_json::json;

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub fn session(id: &str, workspace: &str, now: u64) -> SessionRecord {
    SessionRecord {
        id: id.into(),
        agent: "codex".into(),
        model: Some("gpt".into()),
        workspace: workspace.into(),
        started_at_ms: now - 2_000,
        ended_at_ms: Some(now),
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

pub fn snapshot(workspace: &str, now: u64) -> RepoSnapshotRecord {
    RepoSnapshotRecord {
        id: "snap".into(),
        workspace: workspace.into(),
        head_commit: None,
        dirty_fingerprint: "clean".into(),
        analyzer_version: "test".into(),
        indexed_at_ms: now,
        dirty: false,
        graph_path: String::new(),
    }
}

pub fn facts() -> Vec<FileFact> {
    vec![
        fact("src/a.rs", 10, 3, 2),
        fact("src/b.rs", 5, 9, 4),
        fact("src/c.rs", 1, 1, 1),
    ]
}

fn fact(path: &str, complexity: u32, churn: u32, authors: u32) -> FileFact {
    FileFact {
        snapshot_id: "snap".into(),
        path: path.into(),
        language: "rust".into(),
        bytes: 1,
        loc: 1,
        sloc: 1,
        complexity_total: complexity,
        max_fn_complexity: complexity,
        symbol_count: 1,
        import_count: 0,
        fan_in: 0,
        fan_out: 0,
        churn_30d: churn,
        churn_90d: churn,
        authors_90d: authors,
        last_changed_ms: None,
    }
}

pub fn events(session_id: &str, now: u64) -> Vec<Event> {
    [
        (0, now - 900, EventKind::ToolCall, "bash", "a", "src/a.rs"),
        (1, now - 800, EventKind::ToolResult, "bash", "a", "src/a.rs"),
        (2, now - 700, EventKind::ToolCall, "bash", "b", "src/a.rs"),
        (3, now - 500, EventKind::ToolResult, "bash", "b", "src/a.rs"),
        (
            4,
            now - 400,
            EventKind::ToolCall,
            "read_file",
            "c",
            "src/b.rs",
        ),
        (
            5,
            now - 100,
            EventKind::ToolResult,
            "read_file",
            "c",
            "src/b.rs",
        ),
    ]
    .into_iter()
    .map(|(seq, ts, kind, tool, call, path)| event(session_id, seq, ts, kind, tool, call, path))
    .collect()
}

fn event(
    session_id: &str,
    seq: u64,
    ts_ms: u64,
    kind: EventKind,
    tool: &str,
    call_id: &str,
    path: &str,
) -> Event {
    Event {
        session_id: session_id.into(),
        seq,
        ts_ms,
        ts_exact: true,
        kind,
        source: EventSource::Tail,
        tool: Some(tool.into()),
        tool_call_id: Some(call_id.into()),
        tokens_in: Some(seq as u32 + 1),
        tokens_out: Some(2),
        reasoning_tokens: Some(1),
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
        payload: json!({ "path": path }),
    }
}
