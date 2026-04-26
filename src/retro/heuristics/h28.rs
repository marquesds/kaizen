// SPDX-License-Identifier: AGPL-3.0-or-later
//! H28 — Large revert churn (git lines) when `revert_lines_14d` is recorded.

use crate::retro::types::{Bet, Inputs};

const MIN_REVERT_LINES: i64 = 100;

pub fn run(inputs: &Inputs) -> Vec<Bet> {
    let heavy: Vec<_> = inputs
        .session_outcomes
        .iter()
        .filter_map(|o| {
            let n = o.revert_lines_14d?;
            (n >= MIN_REVERT_LINES).then_some((o.session_id.clone(), n))
        })
        .collect();
    if heavy.is_empty() {
        return vec![];
    }
    let (sid, n) = heavy.iter().max_by_key(|(_, k)| *k).unwrap();
    vec![Bet {
        id: format!("H28:revert:{sid}"),
        heuristic_id: "H28".into(),
        title: "High revert volume in rolling window".into(),
        hypothesis: format!(
            "Session {sid} attributes {n} revert lines (14d); agent may be thrashing."
        ),
        expected_tokens_saved_per_week: (*n as f64) * 2.0,
        effort_minutes: 25,
        evidence: vec![format!("revert_lines_14d={n}")],
        apply_step: "Tighten review; reduce scope per task; align with main more often.".into(),
        evidence_recency_ms: inputs.window_end_ms,
    }]
}
