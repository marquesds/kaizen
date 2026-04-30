// SPDX-License-Identifier: AGPL-3.0-or-later
//! H27 — High automated test failure rate in measured sessions.

use crate::retro::types::{Bet, Inputs};

const FAIL_RATE: f64 = 0.2;
const MIN_RUN: i64 = 5;

pub fn run(inputs: &Inputs) -> Vec<Bet> {
    let bad: Vec<_> = inputs
        .session_outcomes
        .iter()
        .filter_map(|o| {
            let p = o.test_passed.unwrap_or(0);
            let f = o.test_failed.unwrap_or(0);
            let t = p + f;
            if t < MIN_RUN {
                return None;
            }
            let rate = f as f64 / t as f64;
            if rate <= FAIL_RATE {
                return None;
            }
            Some((o.session_id.clone(), rate, f, t))
        })
        .collect();
    if bad.is_empty() {
        return vec![];
    }
    let (sid, rate, fail_n, total) = bad
        .iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
        .unwrap();
    vec![Bet {
        id: format!("H27:fail:{sid}"),
        heuristic_id: "H27".into(),
        title: "High test failure rate in outcome snapshot".into(),
        hypothesis: format!(
            "Session {sid}: {fail_n}/{total} tests failed ({:.0}% failures).",
            rate * 100.0
        ),
        expected_tokens_saved_per_week: 2000.0 * rate,
        effort_minutes: 30,
        evidence: vec![sid.clone()],
        apply_step: "Stabilize tests before long agent runs; fix or quarantine flakies.".into(),
        evidence_recency_ms: inputs.window_end_ms,
        confidence: None,
        category: None,
    }]
}
