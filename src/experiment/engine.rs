// SPDX-License-Identifier: AGPL-3.0-or-later
//! Turn an `Experiment` + sessions into a report. Pure compute given inputs.

use crate::core::event::{Event, SessionRecord};
use crate::experiment::binding::{ManualTags, partition};
use crate::experiment::metric::value_for;
use crate::experiment::stats::sequential::{Decision, decide as seq_decide};
use crate::experiment::stats::{DEFAULT_RESAMPLES, Summary, summarize};
use crate::experiment::types::{Classification, Criterion, Direction, Experiment, GuardrailResult};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub experiment: Experiment,
    pub summary: Summary,
    pub excluded_count: usize,
    pub target_met: Option<bool>,
    pub guardrail_results: Vec<GuardrailResult>,
    /// Sequential decision: sticky once Significant.
    pub sequential_decision: Decision,
    /// Pass this back on the next `run` call to preserve stickiness.
    pub ever_significant: bool,
}

/// Pure ranking step once `sessions` + per-session `events` gathered.
///
/// Pass the previous `ever_significant` from the last report to preserve
/// sequential stickiness across incremental calls.
pub fn run(
    exp: &Experiment,
    sessions: &[(SessionRecord, Vec<Event>)],
    manual_tags: &ManualTags,
    workspace: &Path,
    ever_significant: bool,
) -> Report {
    let records: Vec<SessionRecord> = sessions.iter().map(|(s, _)| s.clone()).collect();
    let (control_s, treatment_s, excluded_s) =
        partition(&records, &exp.binding, manual_tags, workspace);
    let control = metric_values(
        exp,
        sessions,
        &control_s,
        Classification::Control,
        manual_tags,
    );
    let treatment = metric_values(
        exp,
        sessions,
        &treatment_s,
        Classification::Treatment,
        manual_tags,
    );
    let _ = excluded_s;
    let excluded = records.len() - control.len() - treatment.len();
    let summary = summarize(
        &control,
        &treatment,
        stable_seed(&exp.id),
        DEFAULT_RESAMPLES,
    );
    let target_met = evaluate_criterion(&exp.success_criterion, &summary);
    let seq = seq_decide(
        &control,
        &treatment,
        stable_seed(&exp.id),
        DEFAULT_RESAMPLES,
        ever_significant,
    );
    let guardrail_results = exp
        .guardrails
        .iter()
        .map(|g| {
            let gvals_c = metric_values_for(g.metric, sessions, &control_s);
            let gvals_t = metric_values_for(g.metric, sessions, &treatment_s);
            let gs = summarize(&gvals_c, &gvals_t, stable_seed(&exp.id), DEFAULT_RESAMPLES);
            let violated = match g.regression_direction {
                Direction::Increase => gs.ci95_lo.map(|lo| lo > 0.0).unwrap_or(false),
                Direction::Decrease => gs.ci95_hi.map(|hi| hi < 0.0).unwrap_or(false),
            };
            GuardrailResult {
                metric: g.metric,
                delta_pct: gs.delta_pct,
                violated,
            }
        })
        .collect();
    Report {
        experiment: exp.clone(),
        summary,
        excluded_count: excluded,
        target_met,
        guardrail_results,
        sequential_decision: seq.decision,
        ever_significant: seq.ever_significant,
    }
}

fn metric_values(
    exp: &Experiment,
    sessions: &[(SessionRecord, Vec<Event>)],
    picked: &[&SessionRecord],
    _which: Classification,
    _tags: &ManualTags,
) -> Vec<f64> {
    metric_values_for(exp.metric, sessions, picked)
}

fn metric_values_for(
    metric: crate::experiment::types::Metric,
    sessions: &[(SessionRecord, Vec<Event>)],
    picked: &[&SessionRecord],
) -> Vec<f64> {
    let ids: std::collections::HashSet<&str> = picked.iter().map(|s| s.id.as_str()).collect();
    sessions
        .iter()
        .filter(|(s, _)| ids.contains(s.id.as_str()))
        .filter_map(|(s, evs)| value_for(metric, s, evs))
        .collect()
}

/// Success when the 95% CI excludes zero in the declared direction.
/// `delta_pct` is display-only and not used here.
fn evaluate_criterion(c: &Criterion, s: &Summary) -> Option<bool> {
    match c {
        Criterion::Delta { direction, .. } => {
            let lo = s.ci95_lo?;
            let hi = s.ci95_hi?;
            Some(match direction {
                Direction::Decrease => hi < 0.0,
                Direction::Increase => lo > 0.0,
            })
        }
        Criterion::Absolute { metric_value } => {
            let m = s.median_treatment?;
            Some(m <= *metric_value)
        }
    }
}

fn stable_seed(id: &str) -> u64 {
    let mut h: u64 = 1469598103934665603;
    for b in id.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(1099511628211);
    }
    h
}

/// Human-readable markdown per `docs/experiments.md`.
pub fn to_markdown(report: &Report) -> String {
    let e = &report.experiment;
    let s = &report.summary;
    let mut out = String::new();
    out.push_str(&format!("# Experiment: {}\n\n", e.name));
    out.push_str(&format!(
        "State: {:?} · Duration: {}d\nHypothesis: {}\nChange: {}\n\n",
        e.state, e.duration_days, e.hypothesis, e.change_description
    ));
    let (ctl_label, trt_label) = match &e.binding {
        crate::experiment::types::Binding::GitCommit {
            control_commit,
            treatment_commit,
        } => (short(control_commit), short(treatment_commit)),
        crate::experiment::types::Binding::Branch {
            control_branch,
            treatment_branch,
        } => (control_branch.clone(), treatment_branch.clone()),
        crate::experiment::types::Binding::ManualTag { variant_field } => {
            (format!("manual:{}", variant_field), "manual".into())
        }
    };
    out.push_str(&format!(
        "Binding: control {} · treatment {}\nMetric: {}\n\n",
        ctl_label,
        trt_label,
        e.metric.as_str()
    ));
    out.push_str("|          | N  | median | mean |\n|---|---|---|---|\n");
    out.push_str(&format!(
        "| control  | {} | {} | {} |\n",
        s.n_control,
        fmt_opt(s.median_control),
        fmt_opt(s.mean_control),
    ));
    out.push_str(&format!(
        "| treatment| {} | {} | {} |\n\n",
        s.n_treatment,
        fmt_opt(s.median_treatment),
        fmt_opt(s.mean_treatment),
    ));
    if let Some(d) = s.delta_median {
        out.push_str(&format!(
            "Delta (median): {:+.2}{}\n",
            d,
            s.delta_pct
                .map(|p| format!(" ({:+.1}%)", p))
                .unwrap_or_default(),
        ));
    }
    if let (Some(lo), Some(hi)) = (s.ci95_lo, s.ci95_hi) {
        out.push_str(&format!(
            "95% bootstrap CI on delta: [{:+.2}, {:+.2}]\n",
            lo, hi
        ));
    }
    if let Some(met) = report.target_met {
        out.push_str(&format!(
            "Target: {}\n",
            if met { "MET" } else { "not met" }
        ));
    }
    out.push_str(&format!("\nExcluded: {} sessions\n", report.excluded_count));
    if s.small_sample_warning {
        out.push_str("Warning: N per arm < 30 — CI may be unreliable.\n");
    }
    out
}

fn fmt_opt(v: Option<f64>) -> String {
    v.map(|x| format!("{:.2}", x)).unwrap_or_else(|| "—".into())
}

fn short(commit: &str) -> String {
    commit.chars().take(7).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::event::SessionStatus;
    use crate::experiment::types::{Binding, Criterion, Direction, Metric, State};

    fn exp() -> Experiment {
        Experiment {
            id: "e".into(),
            name: "e".into(),
            hypothesis: "h".into(),
            change_description: "c".into(),
            metric: Metric::TokensPerSession,
            binding: Binding::GitCommit {
                control_commit: "c".into(),
                treatment_commit: "t".into(),
            },
            duration_days: 14,
            success_criterion: Criterion::Delta {
                direction: Direction::Decrease,
                target_pct: -10.0,
            },
            state: State::Running,
            created_at_ms: 0,
            concluded_at_ms: None,
            guardrails: Vec::new(),
        }
    }

    fn session_with(id: &str, tokens: u32) -> (SessionRecord, Vec<Event>) {
        let s = SessionRecord {
            id: id.into(),
            agent: "cursor".into(),
            model: None,
            workspace: "/ws".into(),
            started_at_ms: 0,
            ended_at_ms: None,
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
        };
        let mut ev = Event {
            session_id: id.into(),
            seq: 0,
            ts_ms: 0,
            ts_exact: false,
            kind: crate::core::event::EventKind::ToolCall,
            source: crate::core::event::EventSource::Tail,
            tool: None,
            tool_call_id: None,
            tokens_in: Some(tokens),
            tokens_out: None,
            reasoning_tokens: None,
            cost_usd_e6: None,
            stop_reason: None,
            latency_ms: None,
            ttft_ms: None,
            retry_count: None,
            context_used_tokens: None,
            context_max_tokens: None,
            cache_creation_tokens: None,
            cache_read_tokens: None,
            system_prompt_tokens: None,
            payload: serde_json::Value::Null,
        };
        ev.tokens_in = Some(tokens);
        (s, vec![ev])
    }

    #[test]
    fn evaluate_criterion_ci_excludes_zero_decrease() {
        let c = Criterion::Delta {
            direction: Direction::Decrease,
            target_pct: -10.0,
        };
        let s = Summary {
            ci95_lo: Some(-20.0),
            ci95_hi: Some(-5.0),
            ..Default::default()
        };
        assert_eq!(evaluate_criterion(&c, &s), Some(true)); // entire CI < 0

        let s = Summary {
            ci95_lo: Some(-20.0),
            ci95_hi: Some(2.0),
            ..Default::default()
        };
        assert_eq!(evaluate_criterion(&c, &s), Some(false)); // CI straddles zero
    }

    #[test]
    fn evaluate_criterion_ci_excludes_zero_increase() {
        let c = Criterion::Delta {
            direction: Direction::Increase,
            target_pct: 10.0,
        };
        let s = Summary {
            ci95_lo: Some(5.0),
            ci95_hi: Some(20.0),
            ..Default::default()
        };
        assert_eq!(evaluate_criterion(&c, &s), Some(true)); // entire CI > 0

        let s = Summary {
            ci95_lo: Some(-2.0),
            ci95_hi: Some(20.0),
            ..Default::default()
        };
        assert_eq!(evaluate_criterion(&c, &s), Some(false)); // CI straddles zero
    }

    #[test]
    fn manual_tags_drive_partition_without_git() {
        let e = exp();
        let sessions = vec![
            session_with("a", 100),
            session_with("b", 80),
            session_with("c", 200),
            session_with("d", 70),
        ];
        let mut tags = ManualTags::new();
        tags.insert("a".into(), Classification::Control);
        tags.insert("b".into(), Classification::Control);
        tags.insert("c".into(), Classification::Treatment);
        tags.insert("d".into(), Classification::Treatment);
        let r = run(&e, &sessions, &tags, Path::new("/no"), false);
        assert_eq!(r.summary.n_control, 2);
        assert_eq!(r.summary.n_treatment, 2);
    }
}
