// SPDX-License-Identifier: AGPL-3.0-or-later
//! H31 — High resident memory for monitored agent PID.

use crate::retro::types::{Bet, Inputs};

const RSS_BYTES: u64 = 1_000_000_000;

pub fn run(inputs: &Inputs) -> Vec<Bet> {
    let Some(agg) = inputs
        .session_sample_aggs
        .iter()
        .find(|a| a.max_rss_bytes >= RSS_BYTES)
    else {
        return vec![];
    };
    let gb = agg.max_rss_bytes as f64 / 1_000_000_000.0;
    vec![Bet {
        id: format!("H31:mem:{}", agg.session_id),
        heuristic_id: "H31".into(),
        title: "High memory use for agent process during session".into(),
        hypothesis: format!(
            "Peak RSS ~{gb:.1} GB ({} samples) for session {}.",
            agg.sample_count, agg.session_id
        ),
        expected_tokens_saved_per_week: gb * 500.0,
        effort_minutes: 20,
        evidence: vec![agg.session_id.clone()],
        apply_step: "Trim context, restart session, or reduce parallel tool work.".into(),
        evidence_recency_ms: inputs.window_end_ms,
        confidence: None,
        category: None,
    }]
}
