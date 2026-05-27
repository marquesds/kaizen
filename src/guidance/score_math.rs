// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::guidance::score_inputs::ScoreInputs;
use crate::guidance::types::{ArtifactState, GuidanceScoreSlice, GuidanceValidationGate};
use crate::store::SessionOutcomeRow;
use std::collections::{HashMap, HashSet};

pub(super) struct ScoreParts {
    pub total: GuidanceScoreSlice,
    pub train: GuidanceScoreSlice,
    pub validation: GuidanceScoreSlice,
    pub generalization_gap: Option<f64>,
    pub validation_gate: GuidanceValidationGate,
    pub evidence: Vec<String>,
}

pub(super) fn validation_start_ms(start_ms: u64, end_ms: u64) -> u64 {
    start_ms + end_ms.saturating_sub(start_ms) * 7 / 10
}

pub(super) fn state(sessions: u64, min_sessions: u64) -> ArtifactState {
    match (sessions, sessions >= min_sessions) {
        (0, _) => ArtifactState::Stale,
        (_, true) => ArtifactState::Current,
        _ => ArtifactState::InsufficientEvidence,
    }
}

pub(super) fn parts(
    ids: &HashSet<String>,
    i: &ScoreInputs,
    min_sessions: u64,
    validation_start_ms: u64,
) -> ScoreParts {
    let total = score_slice(ids, i);
    let (train, validation) = train_validation(ids, i, validation_start_ms);
    ScoreParts {
        generalization_gap: score_gap(&train, &validation),
        validation_gate: validation_gate(&train, &validation, min_sessions),
        evidence: evidence(&total, &validation),
        total,
        train,
        validation,
    }
}

fn score(sessions: u64, eval: Option<f64>, bad: u64, failed: u64, loops: u64) -> f64 {
    if sessions == 0 {
        return 0.0;
    }
    let eval_penalty = eval.map(|v| (1.0 - v) * 35.0).unwrap_or(12.0);
    let quality_penalty = (bad + failed).min(10) as f64 * 6.0;
    (100.0 - eval_penalty - quality_penalty - loops.min(20) as f64).clamp(0.0, 100.0)
}

fn score_slice(ids: &HashSet<String>, i: &ScoreInputs) -> GuidanceScoreSlice {
    let sessions = ids.len() as u64;
    let mean_eval = mean_eval(ids, &i.evals);
    let bad = ids.iter().filter(|id| i.feedback_bad.contains(*id)).count() as u64;
    let failed = ids
        .iter()
        .filter(|id| outcome_failed(i.outcomes.get(*id)))
        .count() as u64;
    let loops = ids.iter().filter_map(|id| i.loops.get(id)).sum();
    GuidanceScoreSlice {
        score: score(sessions, mean_eval, bad, failed, loops),
        sessions,
        avg_cost_usd: avg_cost(ids, &i.costs),
        mean_eval_score: mean_eval,
        bad_feedback: bad,
        failed_outcomes: failed,
        tool_loops: loops,
    }
}

fn train_validation(
    ids: &HashSet<String>,
    i: &ScoreInputs,
    start_ms: u64,
) -> (GuidanceScoreSlice, GuidanceScoreSlice) {
    let (train, validation) = split_ids(ids, &i.started_at_ms, start_ms);
    (score_slice(&train, i), score_slice(&validation, i))
}

fn split_ids(
    ids: &HashSet<String>,
    started: &HashMap<String, u64>,
    start_ms: u64,
) -> (HashSet<String>, HashSet<String>) {
    ids.iter()
        .cloned()
        .partition(|id| started.get(id).copied().unwrap_or(0) < start_ms)
}

fn score_gap(train: &GuidanceScoreSlice, validation: &GuidanceScoreSlice) -> Option<f64> {
    (train.sessions > 0 && validation.sessions > 0).then_some(validation.score - train.score)
}

fn validation_gate(
    train: &GuidanceScoreSlice,
    validation: &GuidanceScoreSlice,
    min_sessions: u64,
) -> GuidanceValidationGate {
    let min_validation = (min_sessions / 3).max(1);
    match (train.sessions + validation.sessions, validation.sessions) {
        (0, _) => GuidanceValidationGate::NoData,
        (_, n) if train.sessions == 0 || n < min_validation => {
            GuidanceValidationGate::NeedsMoreValidation
        }
        _ if validation.score + 10.0 < train.score => GuidanceValidationGate::Regression,
        _ => GuidanceValidationGate::Stable,
    }
}

fn evidence(total: &GuidanceScoreSlice, validation: &GuidanceScoreSlice) -> Vec<String> {
    [
        format!("{} session(s) referenced artifact", total.sessions),
        total
            .mean_eval_score
            .map(|v| format!("mean eval score {v:.2}"))
            .unwrap_or_default(),
        format!("{} bad/regression feedback record(s)", total.bad_feedback),
        format!("{} failed outcome record(s)", total.failed_outcomes),
        format!("{} repeated tool call(s)", total.tool_loops),
        held_out_evidence(validation),
    ]
    .into_iter()
    .filter(|s| !s.is_empty())
    .collect()
}

fn held_out_evidence(validation: &GuidanceScoreSlice) -> String {
    match validation.sessions {
        0 => "held-out validation has no session(s)".into(),
        n => format!(
            "held-out validation score {:.1} over {n} session(s)",
            validation.score
        ),
    }
}

fn mean_eval(ids: &HashSet<String>, evals: &HashMap<String, Vec<f64>>) -> Option<f64> {
    let vals: Vec<f64> = ids
        .iter()
        .filter_map(|id| evals.get(id))
        .flatten()
        .copied()
        .collect();
    (!vals.is_empty()).then(|| vals.iter().sum::<f64>() / vals.len() as f64)
}

fn avg_cost(ids: &HashSet<String>, costs: &HashMap<String, i64>) -> Option<f64> {
    (!ids.is_empty()).then(|| {
        ids.iter().filter_map(|id| costs.get(id)).sum::<i64>() as f64
            / ids.len() as f64
            / 1_000_000.0
    })
}

fn outcome_failed(row: Option<&SessionOutcomeRow>) -> bool {
    row.is_some_and(|r| {
        r.build_ok == Some(false)
            || r.ci_ok == Some(false)
            || r.test_failed.unwrap_or(0) > 0
            || r.lint_errors.unwrap_or(0) > 0
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn score_penalizes_low_eval_and_bad_feedback() {
        assert!(score(3, Some(0.2), 1, 1, 2) < score(3, Some(0.9), 0, 0, 0));
    }

    #[test]
    fn validation_gate_flags_held_out_regression() {
        let train = GuidanceScoreSlice {
            score: 90.0,
            sessions: 20,
            ..Default::default()
        };
        let validation = GuidanceScoreSlice {
            score: 60.0,
            sessions: 10,
            ..Default::default()
        };
        assert_eq!(
            validation_gate(&train, &validation, 30),
            GuidanceValidationGate::Regression
        );
    }
}
