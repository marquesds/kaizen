use kaizen::core::event::{Event, EventKind, EventSource, SessionRecord, SessionStatus};
use kaizen::store::Store;
use serde_json::json;
use std::time::{Duration, Instant};

const SPANS: usize = 250;

#[test]
#[ignore = "perf harness prints append p99 and sustained ingest"]
fn projector_phase2_perf_harness() -> anyhow::Result<()> {
    unsafe { std::env::remove_var("KAIZEN_PROJECTOR") };
    let dir = tempfile::tempdir()?;
    let store = Store::open(&dir.path().join("kaizen.db"))?;
    store.upsert_session(&session("perf"))?;
    let events = paired_events("perf", SPANS);
    let start = Instant::now();
    let mut samples = Vec::new();
    for event in &events {
        let t = Instant::now();
        store.append_event(event)?;
        samples.push(t.elapsed());
    }
    samples.sort();
    let p99 = samples[samples.len() * 99 / 100];
    let rate = events.len() as f64 / start.elapsed().as_secs_f64();
    eprintln!("phase2 projector perf:");
    eprintln!("  events: {}", events.len());
    eprintln!("  append p99: {}", ms(p99));
    eprintln!("  sustained ingest: {:.0} evt/s", rate);
    Ok(())
}

fn session(id: &str) -> SessionRecord {
    SessionRecord {
        id: id.to_string(),
        agent: "codex".into(),
        model: Some("gpt".into()),
        workspace: "/ws".into(),
        started_at_ms: 0,
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

fn paired_events(session_id: &str, pairs: usize) -> Vec<Event> {
    let mut out = Vec::new();
    for i in 0..pairs {
        out.push(event(session_id, (i * 2) as u64, EventKind::ToolCall, i));
        out.push(event(
            session_id,
            (i * 2 + 1) as u64,
            EventKind::ToolResult,
            i,
        ));
    }
    out
}

fn event(session_id: &str, seq: u64, kind: EventKind, id: usize) -> Event {
    Event {
        session_id: session_id.into(),
        seq,
        ts_ms: 1_000 + seq,
        ts_exact: true,
        kind,
        source: EventSource::Tail,
        tool: Some("bash".into()),
        tool_call_id: Some(format!("call-{id}")),
        tokens_in: Some(10),
        tokens_out: Some(20),
        reasoning_tokens: Some(5),
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
        payload: json!({"path": format!("src/{id}.rs")}),
    }
}

fn ms(duration: Duration) -> String {
    format!("{:.2}ms", duration.as_secs_f64() * 1_000.0)
}
