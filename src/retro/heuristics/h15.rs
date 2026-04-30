// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::retro::types::{Bet, Inputs};

const LOW_THRESHOLD: f64 = 0.4;
const MIN_LOW_COUNT: usize = 3;
const TOKENS_PER_LOW_SESSION: f64 = 600.0;

pub fn run(inputs: &Inputs) -> Vec<Bet> {
    let scores = &inputs.eval_scores;
    if scores.is_empty() {
        return vec![];
    }
    let low_count = scores.iter().filter(|(_, s)| *s < LOW_THRESHOLD).count();
    if low_count < MIN_LOW_COUNT && !mean_dropped(scores) {
        return vec![];
    }
    vec![make_bet(scores, low_count, inputs.window_end_ms)]
}

fn mean_dropped(scores: &[(String, f64)]) -> bool {
    let _ = scores;
    false
}

fn make_bet(scores: &[(String, f64)], low_count: usize, recency_ms: u64) -> Bet {
    Bet {
        id: "H15:low-eval-scores".into(),
        heuristic_id: "H15".into(),
        title: "Low eval scores — agent struggling with tool efficiency".into(),
        hypothesis: format!(
            "{} session(s) scored below {:.0}% on tool-efficiency-v1.",
            low_count,
            LOW_THRESHOLD * 100.0,
        ),
        expected_tokens_saved_per_week: low_count as f64 * TOKENS_PER_LOW_SESSION,
        effort_minutes: 30,
        evidence: low_evidence(scores),
        apply_step: "Run `kaizen eval list --min-score 0` to review low-scoring sessions.".into(),
        evidence_recency_ms: recency_ms,
        confidence: None,
        category: None,
    }
}

fn low_evidence(scores: &[(String, f64)]) -> Vec<String> {
    scores
        .iter()
        .filter(|(_, s)| *s < LOW_THRESHOLD)
        .map(|(id, s)| format!("{}: {:.2}", id, s))
        .collect()
}
