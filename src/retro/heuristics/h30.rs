// SPDX-License-Identifier: AGPL-3.0-or-later
//! H30 — Sustained high CPU for monitored agent PID.

use crate::retro::types::{Bet, Inputs};

const CPU_PCT: f64 = 80.0;

pub fn run(inputs: &Inputs) -> Vec<Bet> {
    let Some(agg) = inputs
        .session_sample_aggs
        .iter()
        .find(|a| a.max_cpu_percent > CPU_PCT)
    else {
        return vec![];
    };
    vec![Bet {
        id: format!("H30:cpu:{}", agg.session_id),
        heuristic_id: "H30".into(),
        title: "High CPU for agent process during session".into(),
        hypothesis: format!(
            "Peak {:.1}% CPU across {} samples for session {}.",
            agg.max_cpu_percent, agg.sample_count, agg.session_id
        ),
        expected_tokens_saved_per_week: agg.max_cpu_percent * 5.0,
        effort_minutes: 15,
        evidence: vec![agg.session_id.clone()],
        apply_step: "Smaller batches; avoid hot loops; check for runaway tool retries.".into(),
        evidence_recency_ms: inputs.window_end_ms,
        confidence: None,
        category: None,
    }]
}
