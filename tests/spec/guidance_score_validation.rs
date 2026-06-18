// SPDX-License-Identifier: AGPL-3.0-or-later
//! `kaizen guidance score --json` exposes held-out validation evidence.

use kaizen::core::event::{Event, EventKind, EventSource, SessionRecord, SessionStatus};
use kaizen::eval::types::EvalRow;
use kaizen::store::Store;
use serde_json::{Value, json};

const DAY_MS: u64 = 86_400_000;

mod test_home;

#[test]
fn score_json_reports_validation_regression() -> anyhow::Result<()> {
    let _home = test_home::TestHome::new()?;
    let tmp = tempfile::tempdir()?;
    let ws = tmp.path().canonicalize()?;
    seed_skill(&ws)?;
    let store = Store::open(&kaizen::core::workspace::db_path(&ws)?)?;
    let now = now_ms();
    seed_sessions(&store, &ws.to_string_lossy(), now - 20 * DAY_MS, 20, 0.95)?;
    seed_sessions(&store, &ws.to_string_lossy(), now - DAY_MS, 10, 0.20)?;

    let text = kaizen::shell::guidance_science::score_text(Some(&ws), 30, 30, true)?;
    let json: Value = serde_json::from_str(&text)?;
    let row = &json["rows"][0];
    assert_eq!(row["artifact"]["slug"], "tdd");
    assert_eq!(row["validation_gate"], "regression");
    assert!(row["generalization_gap"].as_f64().unwrap() < -10.0);
    Ok(())
}

fn seed_skill(ws: &std::path::Path) -> anyhow::Result<()> {
    let dir = ws.join(".cursor/skills/tdd");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("SKILL.md"), "name: tdd\n")?;
    Ok(())
}

fn seed_sessions(
    store: &Store,
    ws: &str,
    start_ms: u64,
    count: u64,
    score: f64,
) -> anyhow::Result<()> {
    for idx in 0..count {
        let id = format!("{start_ms}-{idx}");
        store.upsert_session(&session(&id, ws, start_ms))?;
        store.append_event(&event(&id, start_ms + idx))?;
        store.upsert_eval(&eval(&id, start_ms + idx, score))?;
    }
    Ok(())
}

fn session(id: &str, ws: &str, started_at_ms: u64) -> SessionRecord {
    SessionRecord {
        id: id.into(),
        agent: "codex".into(),
        model: Some("gpt".into()),
        workspace: ws.into(),
        started_at_ms,
        ended_at_ms: Some(started_at_ms + 1),
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

fn event(session_id: &str, ts_ms: u64) -> Event {
    Event {
        session_id: session_id.into(),
        seq: 0,
        ts_ms,
        ts_exact: true,
        kind: EventKind::Message,
        source: EventSource::Tail,
        tool: None,
        tool_call_id: None,
        tokens_in: Some(0),
        tokens_out: Some(0),
        reasoning_tokens: None,
        cost_usd_e6: Some(0),
        stop_reason: None,
        latency_ms: None,
        ttft_ms: None,
        retry_count: None,
        context_used_tokens: None,
        context_max_tokens: None,
        cache_creation_tokens: None,
        cache_read_tokens: None,
        system_prompt_tokens: None,
        payload: json!({"path": ".cursor/skills/tdd/SKILL.md"}),
    }
}

fn eval(session_id: &str, created_at_ms: u64, score: f64) -> EvalRow {
    EvalRow {
        id: format!("eval-{session_id}"),
        session_id: session_id.into(),
        judge_model: "judge".into(),
        rubric_id: "default".into(),
        score,
        rationale: "seed".into(),
        flagged: false,
        created_at_ms,
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
