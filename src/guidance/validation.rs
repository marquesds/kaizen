// SPDX-License-Identifier: AGPL-3.0-or-later
//! Held-out gate for applied guidance candidates.

use crate::experiment::store as exp_store;
use crate::experiment::types::{Experiment, Metric};
use crate::experiment::{self, Report};
use crate::guidance::{CandidateStatus, GuidanceCandidate};
use crate::store::Store;
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValidationGate {
    pub candidate_id: String,
    pub experiment_id: String,
    pub outcome: ValidationOutcome,
    pub n_control: usize,
    pub n_treatment: usize,
    pub delta_pct: Option<f64>,
    pub target_met: Option<bool>,
    pub guardrail_violations: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationOutcome {
    Validated,
    Rejected,
    InsufficientEvidence,
}

impl ValidationGate {
    pub fn next_status(&self) -> Option<CandidateStatus> {
        match self.outcome {
            ValidationOutcome::Validated => Some(CandidateStatus::Validated),
            ValidationOutcome::Rejected => Some(CandidateStatus::Rejected),
            ValidationOutcome::InsufficientEvidence => None,
        }
    }
}

pub fn evaluate(store: &Store, workspace: &Path, c: &GuidanceCandidate) -> Result<ValidationGate> {
    let exp_id = c
        .experiment_id
        .as_deref()
        .ok_or_else(|| anyhow!("candidate has no prompt-bound experiment"))?;
    let exp = exp_store::load_experiment(store, exp_id)?
        .ok_or_else(|| anyhow!("experiment not found: {exp_id}"))?;
    Ok(from_report(c, &report(store, workspace, &exp)?, exp_id))
}

fn from_report(c: &GuidanceCandidate, report: &Report, exp_id: &str) -> ValidationGate {
    let summary = &report.summary;
    let violations = report
        .guardrail_results
        .iter()
        .filter(|g| g.violated)
        .count();
    ValidationGate {
        candidate_id: c.id.clone(),
        experiment_id: exp_id.into(),
        outcome: outcome(
            report.target_met,
            summary.n_control,
            summary.n_treatment,
            violations,
        ),
        n_control: summary.n_control,
        n_treatment: summary.n_treatment,
        delta_pct: summary.delta_pct,
        target_met: report.target_met,
        guardrail_violations: violations,
    }
}

fn outcome(
    target_met: Option<bool>,
    n_control: usize,
    n_treatment: usize,
    guardrails: usize,
) -> ValidationOutcome {
    match (n_control, n_treatment, target_met, guardrails) {
        (0, _, _, _) | (_, 0, _, _) | (_, _, None, _) => ValidationOutcome::InsufficientEvidence,
        (_, _, _, n) if n > 0 => ValidationOutcome::Rejected,
        (_, _, Some(true), _) => ValidationOutcome::Validated,
        (_, _, Some(false), _) => ValidationOutcome::Rejected,
    }
}

fn report(store: &Store, workspace: &Path, exp: &Experiment) -> Result<Report> {
    let ws = workspace.to_string_lossy().to_string();
    let (start, end) = window_for(exp);
    let manual = exp_store::manual_tags(store, &exp.id)?;
    let (sessions, values) = metric_values(store, &ws, start, end, exp.metric)?;
    let guardrails = guardrail_values(store, &ws, start, end, exp)?;
    Ok(experiment::run_from_metric_values(
        exp,
        &sessions,
        &values,
        &guardrails,
        &manual,
        workspace,
        false,
    ))
}

fn guardrail_values(
    store: &Store,
    ws: &str,
    start: u64,
    end: u64,
    exp: &Experiment,
) -> Result<HashMap<Metric, HashMap<String, f64>>> {
    exp.guardrails
        .iter()
        .map(|g| metric_values(store, ws, start, end, g.metric).map(|(_, vals)| (g.metric, vals)))
        .collect()
}

fn metric_values(
    store: &Store,
    ws: &str,
    start: u64,
    end: u64,
    metric: Metric,
) -> Result<(Vec<crate::core::event::SessionRecord>, HashMap<String, f64>)> {
    let rows = store.experiment_metric_values_in_window(ws, start, end, metric)?;
    Ok(rows.into_iter().fold(
        (Vec::new(), HashMap::new()),
        |(mut sessions, mut values), (session, value)| {
            values.insert(session.id.clone(), value);
            sessions.push(session);
            (sessions, values)
        },
    ))
}

fn window_for(e: &Experiment) -> (u64, u64) {
    let end = e
        .concluded_at_ms
        .unwrap_or_else(|| e.created_at_ms + (e.duration_days as u64) * 86_400_000);
    (e.created_at_ms, end.max(e.created_at_ms))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_hit_validates() {
        assert_eq!(outcome(Some(true), 30, 30, 0), ValidationOutcome::Validated);
    }

    #[test]
    fn target_miss_rejects() {
        assert_eq!(outcome(Some(false), 30, 30, 0), ValidationOutcome::Rejected);
    }

    #[test]
    fn missing_arm_keeps_applied() {
        assert_eq!(
            outcome(None, 0, 30, 0),
            ValidationOutcome::InsufficientEvidence
        );
    }
}
