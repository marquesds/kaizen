// SPDX-License-Identifier: AGPL-3.0-or-later
//! Scientific skill/rule scoring and candidate commands.

use crate::experiment::store as exp_store;
use crate::experiment::types::{Binding, Criterion, Direction, Experiment, Metric, State};
use crate::guidance::{self, ArtifactRef, CandidateStatus, GuidanceCandidate};
use crate::shell::cli::workspace_path;
use crate::store::Store;
use anyhow::{Result, anyhow};
use std::fmt::Write;
use std::path::Path;

pub fn cmd_score(ws: Option<&Path>, days: u32, min_sessions: u64, json_out: bool) -> Result<()> {
    print!("{}", score_text(ws, days, min_sessions, json_out)?);
    Ok(())
}

pub fn score_text(
    ws: Option<&Path>,
    days: u32,
    min_sessions: u64,
    json_out: bool,
) -> Result<String> {
    let ws = workspace_path(ws)?;
    let store = Store::open(&crate::core::workspace::db_path(&ws)?)?;
    let ws_key = ws.to_string_lossy().to_string();
    let (start, end) = crate::shell::guidance::trailing_window_ms(days);
    let report = guidance::score::build(&store, &ws, &ws_key, start, end, min_sessions)?;
    if json_out {
        return Ok(serde_json::to_string_pretty(&report)?);
    }
    Ok(format_score(&report))
}

pub fn cmd_propose(
    ws: Option<&Path>,
    artifact: &str,
    max_ops: usize,
    llm: bool,
    apply: bool,
    json_out: bool,
) -> Result<()> {
    print!(
        "{}",
        propose_text(ws, artifact, max_ops, llm, apply, json_out)?
    );
    Ok(())
}

pub fn propose_text(
    ws: Option<&Path>,
    artifact: &str,
    max_ops: usize,
    llm: bool,
    apply: bool,
    json_out: bool,
) -> Result<String> {
    let ws = workspace_path(ws)?;
    let store = Store::open(&crate::core::workspace::db_path(&ws)?)?;
    let artifact =
        ArtifactRef::parse(artifact).ok_or_else(|| anyhow!("use skill:<id> or rule:<id>"))?;
    let mut candidate = proposed_candidate(&ws, &store, &artifact, max_ops, llm)?;
    store.upsert_guidance_candidate(&candidate)?;
    if apply {
        apply_and_record(&ws, &store, &mut candidate)?;
    }
    if json_out {
        Ok(serde_json::to_string_pretty(&candidate)?)
    } else {
        Ok(crate::shell::guidance_candidates::format_candidate(
            &candidate,
        ))
    }
}

fn proposed_candidate(
    ws: &Path,
    store: &Store,
    artifact: &ArtifactRef,
    max_ops: usize,
    llm: bool,
) -> Result<GuidanceCandidate> {
    let row = score_row(ws, store, artifact)?;
    let rejected = store.rejected_guidance_candidates(artifact, 5)?;
    if llm {
        let cfg = crate::core::config::load(ws)?.guidance.proposals;
        return guidance::llm::candidate(&cfg, &row, max_ops, now_ms(), &rejected, ws);
    }
    Ok(guidance::proposals::deterministic(
        &row,
        now_ms(),
        &rejected,
    ))
}

fn score_row(
    ws: &Path,
    store: &Store,
    artifact: &ArtifactRef,
) -> Result<crate::guidance::GuidanceScoreRow> {
    let ws_key = ws.to_string_lossy().to_string();
    let (start, end) = crate::shell::guidance::trailing_window_ms(30);
    let report = guidance::score::build(store, ws, &ws_key, start, end, 30)?;
    report
        .rows
        .into_iter()
        .find(|r| r.artifact == *artifact)
        .ok_or_else(|| anyhow!("artifact not in scorecard: {artifact}"))
}

fn apply_and_record(ws: &Path, store: &Store, candidate: &mut GuidanceCandidate) -> Result<()> {
    let now = now_ms();
    let before = crate::prompt::snapshot::capture(ws, now)?;
    store.upsert_prompt_snapshot(&before)?;
    let applied = guidance::proposals::apply_candidate(ws, candidate, now)?;
    store.upsert_prompt_snapshot(&applied.snapshot)?;
    let exp_id = create_prompt_experiment(
        store,
        candidate,
        &before.fingerprint,
        &applied.treatment_fingerprint,
        now,
    )?;
    store.mark_guidance_candidate_applied(
        &candidate.id,
        now,
        &applied.treatment_fingerprint,
        &applied.backup_path,
        Some(&exp_id),
    )?;
    candidate.status = CandidateStatus::Applied;
    candidate.applied_at_ms = Some(now);
    candidate.treatment_fingerprint = Some(applied.treatment_fingerprint);
    candidate.backup_path = Some(applied.backup_path);
    candidate.experiment_id = Some(exp_id);
    Ok(())
}

fn create_prompt_experiment(
    store: &Store,
    c: &GuidanceCandidate,
    control: &str,
    treatment: &str,
    now: u64,
) -> Result<String> {
    let exp = Experiment {
        id: uuid::Uuid::now_v7().to_string(),
        name: format!("guidance-{}", c.artifact),
        hypothesis: format!("{} improves {}", c.rationale, c.artifact),
        change_description: format!("guidance candidate {}", c.id),
        metric: Metric::CostPerSession,
        binding: Binding::PromptFingerprint {
            control_fingerprint: control.into(),
            treatment_fingerprint: treatment.into(),
        },
        duration_days: 14,
        success_criterion: Criterion::Delta {
            direction: Direction::Decrease,
            target_pct: -10.0,
        },
        state: State::Running,
        created_at_ms: now,
        concluded_at_ms: None,
        guardrails: Vec::new(),
    };
    exp_store::save_experiment(store, &exp)?;
    Ok(exp.id)
}

fn format_score(report: &crate::guidance::GuidanceScoreReport) -> String {
    let mut out = String::new();
    let _ = writeln!(&mut out, "guidance score - {}", report.workspace);
    let _ = writeln!(
        &mut out,
        "{:<8} {:<24} {:>7} {:>7} {:>8} STATE",
        "kind", "id", "score", "val", "sessions"
    );
    for row in &report.rows {
        let _ = writeln!(
            &mut out,
            "{:<8} {:<24} {:>7.1} {:>7.1} {:>8} {:?}",
            row.artifact.kind.as_str(),
            row.artifact.slug,
            row.score,
            row.validation.score,
            row.sessions,
            row.state
        );
    }
    out
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
