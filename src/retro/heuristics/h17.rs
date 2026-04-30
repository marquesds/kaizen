// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::feedback::types::FeedbackLabel;
use crate::retro::types::{Bet, Inputs};

const MIN_SCORED: usize = 5;
const MEAN_THRESHOLD: f64 = 2.5;
const MIN_BAD: usize = 2;
const TOKENS_PER_BAD: f64 = 800.0;

pub fn run(inputs: &Inputs) -> Vec<Bet> {
    let n_bad = count_bad(&inputs.feedback);
    let scored: Vec<u8> = inputs
        .feedback
        .iter()
        .filter_map(|r| r.score.as_ref().map(|s| s.0))
        .collect();
    let mean_opt = mean_score(&scored);
    let fires = n_bad >= MIN_BAD
        || mean_opt.is_some_and(|m| scored.len() >= MIN_SCORED && m <= MEAN_THRESHOLD);
    if !fires {
        return vec![];
    }
    vec![Bet {
        id: "H17:human-feedback".into(),
        heuristic_id: "H17".into(),
        title: "Human feedback signals agent struggles".into(),
        hypothesis: format!(
            "{n_bad} bad/regression feedback records in window (mean={}).",
            mean_opt.map_or("-".into(), |m| format!("{m:.1}"))
        ),
        expected_tokens_saved_per_week: n_bad as f64 * TOKENS_PER_BAD,
        effort_minutes: 30,
        evidence: bad_session_ids(&inputs.feedback),
        apply_step: "Review flagged sessions with `kaizen sessions show <id>`.".into(),
        evidence_recency_ms: inputs.window_end_ms,
        confidence: None,
        category: None,
    }]
}

fn count_bad(fb: &[crate::feedback::types::FeedbackRecord]) -> usize {
    fb.iter()
        .filter(|r| {
            matches!(
                r.label,
                Some(FeedbackLabel::Bad) | Some(FeedbackLabel::Regression)
            )
        })
        .count()
}

fn mean_score(scores: &[u8]) -> Option<f64> {
    if scores.is_empty() {
        return None;
    }
    Some(scores.iter().map(|&s| s as f64).sum::<f64>() / scores.len() as f64)
}

fn bad_session_ids(fb: &[crate::feedback::types::FeedbackRecord]) -> Vec<String> {
    fb.iter()
        .filter(|r| {
            matches!(
                r.label,
                Some(FeedbackLabel::Bad) | Some(FeedbackLabel::Regression)
            )
        })
        .take(5)
        .map(|r| r.session_id.clone())
        .collect()
}
