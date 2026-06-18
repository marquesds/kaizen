use super::cli::persist_session_batch;
use crate::core::event::{Event, EventKind, EventSource, SessionRecord, SessionStatus};
use crate::store::Store;
use serde_json::json;

#[test]
fn scanned_batch_appends_only_unseen_suffix() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(&temp.path().join("kaizen.db")).unwrap();
    persist_session_batch(&store, vec![(record(), vec![event(0), event(1)])], None).unwrap();
    let mut stale = event(0);
    stale.payload = json!({"changed": true});
    let stats =
        persist_session_batch(&store, vec![(record(), vec![stale, event(2)])], None).unwrap();
    let rows = store.list_events_for_session("scan").unwrap();
    assert_eq!((stats.events_found, stats.events_upserted), (2, 1));
    assert_eq!((rows.len(), &rows[0].payload), (3, &json!({})));
}

#[test]
fn repeated_scan_preserves_repo_binding() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(&temp.path().join("kaizen.db")).unwrap();
    persist_session_batch(&store, vec![(record(), Vec::new())], None).unwrap();
    let mut rescanned = record();
    rescanned.start_commit = None;
    persist_session_batch(&store, vec![(rescanned, Vec::new())], None).unwrap();
    let stored = store.get_session("scan").unwrap().unwrap();
    assert_eq!(stored.start_commit.as_deref(), Some("abc"));
}

fn record() -> SessionRecord {
    SessionRecord {
        id: "scan".into(),
        agent: "codex".into(),
        model: None,
        workspace: String::new(),
        started_at_ms: 1,
        ended_at_ms: None,
        status: SessionStatus::Running,
        trace_path: "/trace".into(),
        start_commit: Some("abc".into()),
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

fn event(seq: u64) -> Event {
    Event {
        session_id: "scan".into(),
        seq,
        ts_ms: seq + 1,
        ts_exact: true,
        kind: EventKind::Message,
        source: EventSource::Tail,
        tool: None,
        tool_call_id: None,
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
