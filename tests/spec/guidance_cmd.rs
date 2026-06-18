// SPDX-License-Identifier: AGPL-3.0-or-later
//! `kaizen guidance` JSON shape in an empty workspace.

use kaizen::DataSource;
use kaizen::core::event::{Event, EventKind, EventSource, SessionRecord, SessionStatus};
use kaizen::experiment::store as exp_store;
use kaizen::experiment::types::{Binding, Criterion, Direction, Experiment, Metric, State};
use kaizen::guidance::{ArtifactKind, ArtifactRef, CandidateAction, CandidateStatus};
use kaizen::store::Store;
use serde_json::json;

mod test_home;

#[test]
fn guidance_json_empty_workspace() -> anyhow::Result<()> {
    let _home = test_home::TestHome::new()?;
    let tmp = tempfile::tempdir()?;
    let text = kaizen::shell::guidance::guidance_text(
        Some(tmp.path()),
        7,
        true,
        false,
        DataSource::Local,
    )?;
    assert!(
        text.contains("\"workspace\"") && text.contains("\"rows\""),
        "{}",
        text
    );
    assert!(text.contains("\"sessions_in_window\": 0"));
    Ok(())
}

#[test]
fn guidance_score_json_empty_workspace() -> anyhow::Result<()> {
    let _home = test_home::TestHome::new()?;
    let tmp = tempfile::tempdir()?;
    let text = kaizen::shell::guidance_science::score_text(Some(tmp.path()), 30, 30, true)?;
    assert!(text.contains("\"rows\": []"), "{text}");
    Ok(())
}

#[test]
fn guidance_propose_apply_backs_up_rule() -> anyhow::Result<()> {
    let _home = test_home::TestHome::new()?;
    let tmp = tempfile::tempdir()?;
    let rules = tmp.path().join(".cursor/rules");
    std::fs::create_dir_all(&rules)?;
    std::fs::write(rules.join("dead.mdc"), "rule text")?;
    let text = kaizen::shell::guidance_science::propose_text(
        Some(tmp.path()),
        "rule:dead",
        3,
        false,
        true,
        true,
    )?;
    assert!(text.contains("\"status\": \"applied\""), "{text}");
    assert!(!rules.join("dead.mdc").exists());
    Ok(())
}

#[test]
fn guidance_validate_uses_experiment_gate() -> anyhow::Result<()> {
    let _home = test_home::TestHome::new()?;
    let tmp = tempfile::tempdir()?;
    let ws = tmp.path().canonicalize()?;
    let store = Store::open(&kaizen::core::workspace::db_path(&ws)?)?;
    let exp = experiment();
    exp_store::save_experiment(&store, &exp)?;
    store.upsert_guidance_candidate(&candidate(&exp.id))?;
    seed_cost_sessions(&store, &ws.to_string_lossy())?;
    let text = kaizen::shell::guidance_candidates::text(
        Some(&ws),
        kaizen::shell::guidance_candidates::CandidateOp::Validate { id: "c1".into() },
    )?;
    let got = store.get_guidance_candidate("c1")?.unwrap();
    assert_eq!(got.status, CandidateStatus::Validated, "{text}");
    Ok(())
}

fn experiment() -> Experiment {
    Experiment {
        id: "exp1".into(),
        name: "gate".into(),
        hypothesis: "lower cost".into(),
        change_description: "candidate".into(),
        metric: Metric::CostPerSession,
        binding: Binding::PromptFingerprint {
            control_fingerprint: "control".into(),
            treatment_fingerprint: "treatment".into(),
        },
        duration_days: 1,
        success_criterion: Criterion::Delta {
            direction: Direction::Decrease,
            target_pct: -10.0,
        },
        state: State::Running,
        created_at_ms: 1_000,
        concluded_at_ms: Some(10_000),
        guardrails: Vec::new(),
    }
}

fn candidate(exp_id: &str) -> kaizen::guidance::GuidanceCandidate {
    kaizen::guidance::GuidanceCandidate {
        id: "c1".into(),
        artifact: ArtifactRef {
            kind: ArtifactKind::Rule,
            slug: "style".into(),
        },
        action: CandidateAction::ReviewOnly,
        status: CandidateStatus::Applied,
        rationale: "test".into(),
        evidence: vec![],
        created_at_ms: 1,
        applied_at_ms: Some(2),
        treatment_fingerprint: Some("treatment".into()),
        experiment_id: Some(exp_id.into()),
        backup_path: None,
    }
}

fn seed_cost_sessions(store: &Store, ws: &str) -> anyhow::Result<()> {
    for idx in 0..30 {
        insert_cost_session(store, ws, &format!("c{idx}"), "control", 10_000_000)?;
        insert_cost_session(store, ws, &format!("t{idx}"), "treatment", 1_000_000)?;
    }
    Ok(())
}

fn insert_cost_session(
    store: &Store,
    ws: &str,
    id: &str,
    fingerprint: &str,
    cost: i64,
) -> anyhow::Result<()> {
    store.upsert_session(&session(id, ws, fingerprint))?;
    store.append_event(&event(id, cost))?;
    Ok(())
}

fn session(id: &str, ws: &str, fingerprint: &str) -> SessionRecord {
    SessionRecord {
        id: id.into(),
        agent: "codex".into(),
        model: Some("gpt".into()),
        workspace: ws.into(),
        started_at_ms: 2_000,
        ended_at_ms: Some(3_000),
        status: SessionStatus::Done,
        trace_path: String::new(),
        start_commit: None,
        end_commit: None,
        branch: None,
        dirty_start: None,
        dirty_end: None,
        repo_binding_source: None,
        prompt_fingerprint: Some(fingerprint.into()),
        parent_session_id: None,
        agent_version: None,
        os: None,
        arch: None,
        repo_file_count: None,
        repo_total_loc: None,
    }
}

fn event(session_id: &str, cost: i64) -> Event {
    Event {
        session_id: session_id.into(),
        seq: 0,
        ts_ms: 2_500,
        ts_exact: true,
        kind: EventKind::Message,
        source: EventSource::Tail,
        tool: None,
        tool_call_id: None,
        tokens_in: Some(0),
        tokens_out: Some(0),
        reasoning_tokens: None,
        cost_usd_e6: Some(cost),
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
