// SPDX-License-Identifier: AGPL-3.0-or-later

use kaizen::core::event::{Event, EventKind, EventSource, SessionRecord, SessionStatus};
use kaizen::store::Store;
use kaizen::visualization::{DerivedStatus, VisualizationQuery, build_report};
use serde_json::json;

#[test]
fn report_rolls_up_cache_and_selected_trace() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let store = Store::open(&tmp.path().join("k.db"))?;
    store.upsert_session(&session("s1", SessionStatus::Running))?;
    store.append_event(&event("s1", 0, EventKind::ToolCall))?;
    store.append_event(&event("s1", 1, EventKind::Error))?;

    let report = build_report(&store, query(Some("s1")))?;

    assert_eq!(report.totals.session_count, 1);
    assert_eq!(report.totals.tokens.cache_read, 10);
    assert_eq!(report.totals.tokens.cache_create, 4);
    assert_eq!(report.sessions[0].status, DerivedStatus::Errored);
    let selected = report.selected.unwrap();
    assert_eq!(selected.events.len(), 2);
    assert_eq!(selected.files, vec!["src/lib.rs"]);
    Ok(())
}

#[test]
fn empty_workspace_returns_empty_report() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let store = Store::open(&tmp.path().join("k.db"))?;
    let report = build_report(&store, query(None))?;
    assert_eq!(report.totals.session_count, 0);
    assert!(!report.quality.warnings.is_empty());
    Ok(())
}

fn query(selected: Option<&str>) -> VisualizationQuery {
    VisualizationQuery {
        workspace: "/ws".into(),
        selected_session_id: selected.map(str::to_string),
        now_ms: 10_000,
        day_start_hour: 7,
    }
}

fn session(id: &str, status: SessionStatus) -> SessionRecord {
    SessionRecord {
        id: id.into(),
        agent: "codex".into(),
        model: Some("gpt-4o".into()),
        workspace: "/ws".into(),
        started_at_ms: 1_000,
        ended_at_ms: None,
        status,
        trace_path: "/trace".into(),
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

fn event(session_id: &str, seq: u64, kind: EventKind) -> Event {
    Event {
        session_id: session_id.into(),
        seq,
        ts_ms: 2_000 + seq,
        ts_exact: true,
        kind,
        source: EventSource::Tail,
        tool: Some("read_file".into()),
        tool_call_id: Some(format!("call-{seq}")),
        tokens_in: Some(20),
        tokens_out: Some(5),
        reasoning_tokens: Some(2),
        cost_usd_e6: Some(123),
        stop_reason: None,
        latency_ms: None,
        ttft_ms: None,
        retry_count: None,
        context_used_tokens: None,
        context_max_tokens: None,
        cache_creation_tokens: Some(2),
        cache_read_tokens: Some(5),
        system_prompt_tokens: None,
        payload: json!({"input": {"path": "src/lib.rs"}}),
    }
}
