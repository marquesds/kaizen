use crate::core::event::{Event, EventKind, EventSource, SessionRecord, SessionStatus};
use serde_json::json;

mod events;
mod maintenance;
mod reports;
mod schema;
mod sessions;
mod spans;

fn make_session(id: &str) -> SessionRecord {
    SessionRecord {
        id: id.to_string(),
        agent: "cursor".to_string(),
        model: None,
        workspace: "/ws".to_string(),
        started_at_ms: 1000,
        ended_at_ms: None,
        status: SessionStatus::Done,
        trace_path: "/trace".to_string(),
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

fn make_event(session_id: &str, seq: u64) -> Event {
    Event {
        session_id: session_id.to_string(),
        seq,
        ts_ms: 1000 + seq * 100,
        ts_exact: false,
        kind: EventKind::ToolCall,
        source: EventSource::Tail,
        tool: Some("read_file".to_string()),
        tool_call_id: Some(format!("call_{seq}")),
        tokens_in: None,
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
        payload: json!({}),
    }
}
