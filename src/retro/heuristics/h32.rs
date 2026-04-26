// SPDX-License-Identifier: AGPL-3.0-or-later
//! H32 — Long-run sampling: many process samples in one session.

use crate::retro::types::{Bet, Inputs};

const MANY_SAMPLES: u64 = 100;

pub fn run(inputs: &Inputs) -> Vec<Bet> {
    let Some(agg) = inputs
        .session_sample_aggs
        .iter()
        .find(|a| a.sample_count >= MANY_SAMPLES)
    else {
        return vec![];
    };
    vec![Bet {
        id: format!("H32:long:{}", agg.session_id),
        heuristic_id: "H32".into(),
        title: "Long agent session (many sampler ticks)".into(),
        hypothesis: format!(
            "{} samples for session {} — long-running or slow tail.",
            agg.sample_count, agg.session_id
        ),
        expected_tokens_saved_per_week: agg.sample_count as f64 * 2.0,
        effort_minutes: 10,
        evidence: vec![agg.session_id.clone()],
        apply_step: "Break work into shorter sessions; checkpoint progress.".into(),
        evidence_recency_ms: inputs.window_end_ms,
    }]
}
