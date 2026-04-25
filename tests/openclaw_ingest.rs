// SPDX-License-Identifier: AGPL-3.0-or-later
//! Integration tests for OpenClaw tail ingester.
//! Uses `scan_openclaw_at` directly to avoid env-var races between parallel tests.

use kaizen::collect::tail::openclaw::scan_openclaw_at;
use std::fs;
use tempfile::TempDir;

fn make_state_dir(ws: &std::path::Path) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    let sessions_dir = dir.path().join("agents/agent-abc/sessions");
    fs::create_dir_all(&sessions_dir).unwrap();
    let ws_str = ws.to_string_lossy();

    let sessions_json = include_str!("fixtures/openclaw/agents/agent-abc/sessions/sessions.json");
    fs::write(sessions_dir.join("sessions.json"), sessions_json).unwrap();

    for (name, template) in [
        (
            "sess-anthropic.jsonl",
            include_str!("fixtures/openclaw/agents/agent-abc/sessions/sess-anthropic.jsonl"),
        ),
        (
            "sess-openai.jsonl",
            include_str!("fixtures/openclaw/agents/agent-abc/sessions/sess-openai.jsonl"),
        ),
        (
            "sess-local.jsonl",
            include_str!("fixtures/openclaw/agents/agent-abc/sessions/sess-local.jsonl"),
        ),
        (
            "sess-other.jsonl",
            include_str!("fixtures/openclaw/agents/agent-abc/sessions/sess-other.jsonl"),
        ),
    ] {
        fs::write(
            sessions_dir.join(name),
            template.replace("__WORKSPACE__", &ws_str),
        )
        .unwrap();
    }
    dir
}

#[test]
fn openclaw_agent_name_and_basic_ingest() {
    let ws_dir = tempfile::tempdir().unwrap();
    let state_dir = make_state_dir(ws_dir.path());

    let sessions = scan_openclaw_at(state_dir.path(), ws_dir.path()).unwrap();
    assert!(!sessions.is_empty(), "expected at least one session");
    for (record, _) in &sessions {
        assert_eq!(record.agent, "openclaw");
    }
}

#[test]
fn openclaw_workspace_filter_rejects_wrong_cwd() {
    let ws_dir = tempfile::tempdir().unwrap();
    let state_dir = make_state_dir(ws_dir.path());

    let sessions = scan_openclaw_at(state_dir.path(), ws_dir.path()).unwrap();
    let ids: Vec<_> = sessions.iter().map(|(r, _)| r.id.as_str()).collect();
    assert!(
        !ids.contains(&"sess-other"),
        "sess-other has wrong cwd and must be rejected"
    );
}

#[test]
fn openclaw_anthropic_model_and_tokens() {
    let ws_dir = tempfile::tempdir().unwrap();
    let state_dir = make_state_dir(ws_dir.path());

    let sessions = scan_openclaw_at(state_dir.path(), ws_dir.path()).unwrap();
    let (rec, events) = sessions
        .iter()
        .find(|(r, _)| r.id == "sess-anthropic")
        .expect("sess-anthropic missing");
    assert_eq!(rec.model.as_deref(), Some("claude-sonnet-4"));
    let tok_ev = events
        .iter()
        .find(|e| e.tokens_in.is_some())
        .expect("expected token event");
    assert_eq!(tok_ev.tokens_in, Some(1200));
    assert_eq!(tok_ev.tokens_out, Some(300));
}

#[test]
fn openclaw_openai_model_ingested() {
    let ws_dir = tempfile::tempdir().unwrap();
    let state_dir = make_state_dir(ws_dir.path());

    let sessions = scan_openclaw_at(state_dir.path(), ws_dir.path()).unwrap();
    let (rec, _) = sessions
        .iter()
        .find(|(r, _)| r.id == "sess-openai")
        .expect("sess-openai missing");
    assert_eq!(rec.model.as_deref(), Some("gpt-4o"));
}

#[test]
fn openclaw_local_model_no_usage_tokens() {
    let ws_dir = tempfile::tempdir().unwrap();
    let state_dir = make_state_dir(ws_dir.path());

    let sessions = scan_openclaw_at(state_dir.path(), ws_dir.path()).unwrap();
    let (rec, events) = sessions
        .iter()
        .find(|(r, _)| r.id == "sess-local")
        .expect("sess-local missing");
    assert_eq!(rec.model.as_deref(), Some("llama3-local"));
    assert!(
        events.iter().all(|e| e.tokens_in.is_none()),
        "local model has no usage"
    );
}

#[test]
fn openclaw_channel_meta_stored_on_events() {
    let ws_dir = tempfile::tempdir().unwrap();
    let state_dir = make_state_dir(ws_dir.path());

    let sessions = scan_openclaw_at(state_dir.path(), ws_dir.path()).unwrap();
    let (_, events) = sessions
        .iter()
        .find(|(r, _)| r.id == "sess-anthropic")
        .expect("sess-anthropic missing");
    for ev in events {
        let channel = ev.payload.pointer("/meta/channel").and_then(|v| v.as_str());
        assert_eq!(channel, Some("dm"), "channel meta should be 'dm'");
    }
}

#[test]
fn openclaw_hook_parse_fixture() {
    use kaizen::collect::hooks::EventKind;
    use kaizen::collect::hooks::openclaw::parse_openclaw_hook;
    let json = include_str!("fixtures/hooks/openclaw_event.json");
    let ev = parse_openclaw_hook(json.trim()).unwrap();
    assert_eq!(ev.kind, EventKind::Stop);
    assert_eq!(ev.session_id, "oc-fixture-sess");
}
