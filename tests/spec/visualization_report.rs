// SPDX-License-Identifier: AGPL-3.0-or-later

use kaizen::core::event::{Event, EventKind, EventSource, SessionRecord, SessionStatus};
use kaizen::store::Store;
use kaizen::visualization::{DerivedStatus, VisualizationLimits, VisualizationQuery, build_report};
use serde_json::json;

#[path = "visualization_report/search.rs"]
mod visualization_search;

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

#[test]
fn report_skips_activity_when_disabled() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let store = Store::open(&tmp.path().join("k.db"))?;
    let mut query = query(None);
    query.include_activity = false;
    let report = build_report(&store, query)?;
    assert!(report.activity.day_bins.is_empty());
    assert!(report.activity.week_bins.is_empty());
    Ok(())
}

#[test]
fn active_now_excludes_stale_open_sessions() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let store = Store::open(&tmp.path().join("k.db"))?;
    store.upsert_session(&session("active", SessionStatus::Running))?;
    store.upsert_session(&session("stale", SessionStatus::Running))?;
    store.append_event(&event_at("active", 0, 999_500))?;
    store.append_event(&event_at("stale", 0, 1))?;
    let mut query = query(None);
    query.now_ms = 1_000_000;

    let report = build_report(&store, query)?;

    assert_eq!(report.totals.running_count, 1);
    Ok(())
}

#[test]
fn legacy_hook_spans_feed_tool_insights() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let store = Store::open(&tmp.path().join("k.db"))?;
    store.upsert_session(&session("legacy", SessionStatus::Done))?;
    store.append_event(&legacy_hook("legacy", 0, "PreToolUse"))?;
    store.append_event(&legacy_hook("legacy", 1, "PostToolUse"))?;

    let report = build_report(&store, query(None))?;

    assert_eq!(report.totals.tool_call_count, 1);
    assert_eq!(report.sessions[0].top_tools, vec![("Read".into(), 1)]);
    Ok(())
}

#[test]
fn activity_bins_preserve_time_boundaries() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let store = Store::open(&tmp.path().join("k.db"))?;
    store.upsert_session(&session("s1", SessionStatus::Done))?;
    for (seq, ts) in [0, 299_999, 300_000, 86_399_999, 86_400_000]
        .into_iter()
        .enumerate()
    {
        store.append_event(&event_at("s1", seq as u64, ts))?;
    }
    let mut query = query(None);
    query.now_ms = 86_400_000;
    let report = build_report(&store, query)?;
    assert_eq!(report.activity.day_bins.len(), 288);
    assert_eq!(report.activity.day_bins[0].event_count, 2);
    assert_eq!(report.activity.day_bins[1].event_count, 1);
    assert_eq!(report.activity.day_bins[287].event_count, 1);
    assert_eq!(
        report
            .activity
            .day_bins
            .iter()
            .map(|b| b.event_count)
            .sum::<u64>(),
        4
    );
    Ok(())
}

fn query(selected: Option<&str>) -> VisualizationQuery {
    VisualizationQuery {
        workspace: "/ws".into(),
        selected_session_id: selected.map(str::to_string),
        now_ms: 10_000,
        include_activity: true,
        select_latest: false,
        session_search: Default::default(),
        limits: VisualizationLimits {
            sessions: 100,
            selected_events: 100,
            selected_spans: 100,
            selected_files: 100,
        },
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
    let mut event = event_at(session_id, seq, 2_000 + seq);
    event.kind = kind;
    event
}

fn event_at(session_id: &str, seq: u64, ts_ms: u64) -> Event {
    Event {
        session_id: session_id.into(),
        seq,
        ts_ms,
        ts_exact: true,
        kind: EventKind::ToolCall,
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

fn legacy_hook(session_id: &str, seq: u64, name: &str) -> Event {
    let mut event = event_at(session_id, seq, 2_000 + seq);
    event.kind = EventKind::Hook;
    event.source = EventSource::Hook;
    event.tool = None;
    event.tool_call_id = Some("legacy-call".into());
    event.payload = json!({"hook_event_name":name,"tool_name":"Read"});
    event
}
