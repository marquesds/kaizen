// SPDX-License-Identifier: AGPL-3.0-or-later
//! H29 — Lint debt or failed tests in single outcome snapshot.

use crate::retro::types::{Bet, Inputs};

const LINT_TRIGGER: i64 = 20;

pub fn run(inputs: &Inputs) -> Vec<Bet> {
    let Some(o) = inputs.session_outcomes.iter().find(|r| {
        r.lint_errors.unwrap_or(0) > LINT_TRIGGER
            || r.test_failed.unwrap_or(0) > 0
                && r.test_passed.unwrap_or(0) + r.test_failed.unwrap_or(0) > 0
    }) else {
        return vec![];
    };
    let lint = o.lint_errors.unwrap_or(0);
    let tf = o.test_failed.unwrap_or(0);
    vec![Bet {
        id: format!("H29:quality:{}", o.session_id),
        heuristic_id: "H29".into(),
        title: "Test or lint debt in post-stop outcome".into(),
        hypothesis: format!(
            "Session {}: lint_errors={lint}, test_failed={tf}.",
            o.session_id
        ),
        expected_tokens_saved_per_week: 800.0 + lint as f64 * 10.0,
        effort_minutes: 20,
        evidence: vec![o.session_id.clone()],
        apply_step: "Run clippy/tests locally before long agent tasks; fix top errors first."
            .into(),
        evidence_recency_ms: inputs.window_end_ms,
    }]
}
