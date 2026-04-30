// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::retro::types::{Bet, Inputs};

const TRUNCATION_RATE_THRESHOLD: f64 = 0.10;

pub fn run(inputs: &Inputs) -> Vec<Bet> {
    let (truncated, total) = count_stop_reasons(inputs);
    if total == 0 {
        return vec![];
    }
    let rate = truncated as f64 / total as f64;
    if rate < TRUNCATION_RATE_THRESHOLD {
        return vec![];
    }
    vec![Bet {
        id: format!("H22:truncation:{truncated}:{total}"),
        heuristic_id: "H22".into(),
        title: format!(
            "Output truncated in {:.0}% of turns ({truncated}/{total})",
            rate * 100.0
        ),
        hypothesis: format!(
            "{truncated} of {total} turns hit max_tokens. Tasks are too large for one turn."
        ),
        expected_tokens_saved_per_week: truncated as f64 * 800.0,
        effort_minutes: 30,
        evidence: vec![format!(
            "{truncated}/{total} turns with stop_reason=max_tokens"
        )],
        apply_step: "Increase output budget (max_tokens) or decompose tasks into smaller turns."
            .into(),
        evidence_recency_ms: inputs.window_end_ms,
        confidence: None,
        category: None,
    }]
}

fn count_stop_reasons(inputs: &Inputs) -> (usize, usize) {
    let mut truncated = 0usize;
    let mut total = 0usize;
    for (_, event) in &inputs.events {
        let Some(ref sr) = event.stop_reason else {
            continue;
        };
        total += 1;
        if sr == "max_tokens" {
            truncated += 1;
        }
    }
    (truncated, total)
}
