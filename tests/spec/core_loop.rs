// SPDX-License-Identifier: AGPL-3.0-or-later
use kaizen::core::event::{Event, EventKind, EventSource, SessionRecord, SessionStatus};
use kaizen::core_loop::{AlertSeverity, RuleAction};
use kaizen::eval::types::EvalRow;
use kaizen::feedback::types::{FeedbackLabel, FeedbackRecord, FeedbackScore};
use kaizen::store::Store;
use serde_json::json;
use tempfile::TempDir;

#[test]
fn structured_query_matches_tool_and_tokens() -> anyhow::Result<()> {
    let (store, ws) = fixture()?;
    let hits = kaizen::core_loop::query::run(&store, &ws, "tool:bash AND tokens_total:>5", 0, 10)?;
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].session_id, "s1");
    Ok(())
}

#[test]
fn structured_query_matches_hook_payload_tools() -> anyhow::Result<()> {
    let (store, ws) = fixture()?;
    store.append_event(&hook_tool_event())?;
    let hits = kaizen::core_loop::query::run(&store, &ws, "tool:bash", 0, 10)?;
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[1].summary, "bash");
    Ok(())
}

#[test]
fn cases_mine_is_idempotent_for_eval_and_feedback() -> anyhow::Result<()> {
    let (store, _) = fixture()?;
    store.upsert_eval(&eval())?;
    store.upsert_feedback(&feedback())?;
    assert_eq!(kaizen::core_loop::cases::mine(&store, 0, 3)?.len(), 2);
    assert_eq!(kaizen::core_loop::cases::mine(&store, 0, 4)?.len(), 2);
    assert_eq!(kaizen::core_loop::cases::list(&store, None)?.len(), 2);
    Ok(())
}

#[test]
fn local_rule_actions_are_idempotent() -> anyhow::Result<()> {
    let (store, ws) = fixture()?;
    let action = RuleAction::EmitAlert {
        severity: AlertSeverity::Warning,
    };
    kaizen::core_loop::rules::create(&store, "shell", "tool:bash", action, 2)?;
    let first = kaizen::core_loop::rules::run_enabled(&store, &ws, 0, 3, false)?;
    let second = kaizen::core_loop::rules::run_enabled(&store, &ws, 0, 4, false)?;
    assert_eq!(first[0].actions, 1);
    assert_eq!(second[0].actions, 1);
    assert_eq!(kaizen::core_loop::alerts::list(&store)?.len(), 1);
    Ok(())
}

fn fixture() -> anyhow::Result<(Store, String)> {
    let dir = TempDir::new()?.keep();
    let store = Store::open(&dir.join("kaizen.db"))?;
    let ws = dir.to_string_lossy().to_string();
    store.upsert_session(&session(&ws))?;
    store.append_event(&event())?;
    Ok((store, ws))
}

fn session(ws: &str) -> SessionRecord {
    SessionRecord {
        id: "s1".into(),
        agent: "codex".into(),
        model: Some("m".into()),
        workspace: ws.into(),
        started_at_ms: 1,
        ended_at_ms: None,
        status: SessionStatus::Done,
        trace_path: "".into(),
        start_commit: None,
        end_commit: None,
        branch: None,
        dirty_start: None,
        dirty_end: None,
        repo_binding_source: None,
        prompt_fingerprint: Some("fp1".into()),
        parent_session_id: None,
        agent_version: None,
        os: None,
        arch: None,
        repo_file_count: None,
        repo_total_loc: None,
    }
}

fn event() -> Event {
    Event {
        session_id: "s1".into(),
        seq: 0,
        ts_ms: 2,
        ts_exact: true,
        kind: EventKind::ToolCall,
        source: EventSource::Hook,
        tool: Some("bash".into()),
        tool_call_id: Some("c1".into()),
        tokens_in: Some(10),
        tokens_out: None,
        reasoning_tokens: None,
        cost_usd_e6: Some(10),
        stop_reason: None,
        latency_ms: None,
        ttft_ms: None,
        retry_count: None,
        context_used_tokens: None,
        context_max_tokens: None,
        cache_creation_tokens: None,
        cache_read_tokens: None,
        system_prompt_tokens: None,
        payload: json!({ "path": "src/main.rs" }),
    }
}

fn hook_tool_event() -> Event {
    Event {
        tool: None,
        seq: 1,
        payload: json!({ "tool_name": "bash" }),
        ..event()
    }
}

fn eval() -> EvalRow {
    EvalRow {
        id: "e1".into(),
        session_id: "s1".into(),
        judge_model: "judge".into(),
        rubric_id: "r1".into(),
        score: 0.2,
        rationale: "slow".into(),
        flagged: true,
        created_at_ms: 2,
    }
}

fn feedback() -> FeedbackRecord {
    FeedbackRecord {
        id: "f1".into(),
        session_id: "s1".into(),
        score: Some(FeedbackScore(1)),
        label: Some(FeedbackLabel::Bad),
        note: None,
        created_at_ms: 2,
    }
}
