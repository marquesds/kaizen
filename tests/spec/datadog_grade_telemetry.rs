// SPDX-License-Identifier: AGPL-3.0-or-later
use kaizen::core::event::{Event, EventKind, EventSource, SessionRecord, SessionStatus};
use kaizen::core::trace_span::{TraceSpanKind, TraceSpanRecord};
use kaizen::metrics::quality::build_quality_report;
use kaizen::store::Store;
use serde_json::json;

#[test]
fn trace_spans_round_trip_beside_existing_events() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let store = Store::open(&tmp.path().join("kaizen.db"))?;
    store.upsert_session(&session("s1"))?;
    store.upsert_trace_span(&TraceSpanRecord {
        span_id: "llm-1".into(),
        trace_id: "trace-s1".into(),
        parent_span_id: Some("step-1".into()),
        session_id: "s1".into(),
        kind: TraceSpanKind::Llm,
        name: "openai.responses".into(),
        status: "ok".into(),
        started_at_ms: Some(1000),
        ended_at_ms: Some(1250),
        duration_ms: Some(250),
        model: Some("gpt-5.2".into()),
        tool: None,
        tokens_in: Some(10),
        tokens_out: Some(20),
        reasoning_tokens: Some(3),
        cost_usd_e6: Some(42),
        context_used_tokens: Some(13),
        context_max_tokens: Some(128_000),
        payload: json!({"provider":"openai","stream":true}),
    })?;

    let spans = store.trace_spans_for_session("s1")?;
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].kind, TraceSpanKind::Llm);
    assert_eq!(spans[0].parent_span_id.as_deref(), Some("step-1"));
    assert_eq!(spans[0].tokens_out, Some(20));
    Ok(())
}

#[test]
fn capture_quality_reports_field_coverage_and_orphans() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let store = Store::open(&tmp.path().join("kaizen.db"))?;
    store.upsert_session(&session("s1"))?;
    store.append_event(&event("s1", 0, EventSource::Proxy, Some(7), Some(90)))?;
    store.append_event(&event("s1", 1, EventSource::Proxy, None, None))?;
    store.append_event(&event("s1", 2, EventSource::Tail, Some(3), None))?;
    store.upsert_trace_span(&TraceSpanRecord {
        span_id: "step-s1-3".into(),
        trace_id: "trace-s1".into(),
        parent_span_id: None,
        session_id: "s1".into(),
        kind: TraceSpanKind::Step,
        name: "turn".into(),
        status: "ok".into(),
        started_at_ms: Some(900),
        ended_at_ms: Some(1200),
        duration_ms: Some(300),
        model: None,
        tool: None,
        tokens_in: None,
        tokens_out: None,
        reasoning_tokens: None,
        cost_usd_e6: None,
        context_used_tokens: None,
        context_max_tokens: None,
        payload: json!({}),
    })?;
    store.upsert_trace_span(&TraceSpanRecord::llm_proxy(
        "s1",
        3,
        1000,
        1100,
        Some("gpt-5.2".into()),
        json!({}),
    ))?;

    let report = build_quality_report(&store, "/ws", 0, 2_000)?;
    assert_eq!(report.events_total, 3);
    assert_eq!(report.proxy_events, 2);
    assert_eq!(report.token_coverage_pct, 67);
    assert_eq!(report.cost_coverage_pct, 33);
    assert_eq!(report.latency_coverage_pct, 33);
    assert_eq!(report.context_coverage_pct, 33);
    assert_eq!(report.proxy_correlation_pct, 50);
    assert_eq!(report.orphan_span_count, 0);
    Ok(())
}

fn session(id: &str) -> SessionRecord {
    SessionRecord {
        id: id.into(),
        agent: "codex".into(),
        model: Some("gpt-5.2".into()),
        workspace: "/ws".into(),
        started_at_ms: 1,
        ended_at_ms: Some(2_000),
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

fn event(
    session_id: &str,
    seq: u64,
    source: EventSource,
    tokens_in: Option<u32>,
    latency_ms: Option<u32>,
) -> Event {
    let has_proxy_context = matches!(source, EventSource::Proxy);
    Event {
        session_id: session_id.into(),
        seq,
        ts_ms: 1_000 + seq,
        ts_exact: true,
        kind: EventKind::Cost,
        source,
        tool: None,
        tool_call_id: None,
        tokens_in,
        tokens_out: None,
        reasoning_tokens: None,
        cost_usd_e6: (seq == 0).then_some(42),
        stop_reason: None,
        latency_ms,
        ttft_ms: None,
        retry_count: Some(0),
        context_used_tokens: has_proxy_context.then_some(tokens_in).flatten(),
        context_max_tokens: has_proxy_context
            .then_some(tokens_in.map(|_| 128_000))
            .flatten(),
        cache_creation_tokens: None,
        cache_read_tokens: None,
        system_prompt_tokens: None,
        payload: json!({}),
    }
}
