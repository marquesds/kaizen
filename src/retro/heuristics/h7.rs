// SPDX-License-Identifier: AGPL-3.0-or-later
//! H7 — Model routing: premium model with very low per-session cost (possible overkill).

use crate::retro::types::{Bet, Inputs};

const MIN_SESSIONS: usize = 8;
const MAX_AVG_COST_E6: i64 = 50_000;

pub fn run(inputs: &Inputs) -> Vec<Bet> {
    if inputs.aggregates.unique_session_ids.len() < MIN_SESSIONS {
        return vec![];
    }
    let n = inputs.aggregates.unique_session_ids.len() as i64;
    let avg = inputs.aggregates.total_cost_usd_e6 / n.max(1);
    if avg > MAX_AVG_COST_E6 {
        return vec![];
    }
    let mut premium = 0u64;
    let mut model_label = String::new();
    for (m, c) in &inputs.aggregates.model_session_counts {
        let ml = m.to_lowercase();
        if ml.contains("sonnet") || ml.contains("opus") || ml.contains("gpt-4") {
            premium += c;
            if model_label.is_empty() {
                model_label = m.clone();
            }
        }
    }
    if premium * 3 < inputs.aggregates.unique_session_ids.len() as u64 {
        return vec![];
    }
    vec![Bet {
        id: "H7:routing".into(),
        heuristic_id: "H7".into(),
        title: "Revisit default model routing".into(),
        hypothesis: format!(
            "Average cost per session is low (~${:.4}) while many sessions use premium models — cheaper defaults may suffice for mechanical work.",
            (avg as f64) / 1_000_000.0
        ),
        expected_tokens_saved_per_week: (n as f64) * 400.0,
        effort_minutes: 25,
        evidence: vec![
            format!("Dominant premium bucket: {}", model_label),
            format!("Sessions sampled: {}", n),
        ],
        apply_step:
            "Route trivial read/refactor tasks to a smaller model; keep premium for architecture."
                .into(),
        evidence_recency_ms: inputs.window_end_ms,
        confidence: None,
        category: None,
    }]
}
