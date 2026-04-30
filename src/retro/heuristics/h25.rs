// SPDX-License-Identifier: AGPL-3.0-or-later
//! H25 — Mode thrash: many mode_transition lifecycle events per session.

use crate::core::event::EventKind;
use crate::retro::types::{Bet, Inputs};
use std::collections::HashMap;

const MIN_AVG_TRANSITIONS: f64 = 4.0;
const MIN_SESSIONS_WITH_SIGNAL: usize = 2;

pub fn run(inputs: &Inputs) -> Vec<Bet> {
    let mut by_session: HashMap<String, u32> = HashMap::new();
    for (_, e) in &inputs.events {
        if e.kind != EventKind::Lifecycle {
            continue;
        }
        if e.payload.get("type").and_then(|t| t.as_str()) != Some("mode_transition") {
            continue;
        }
        *by_session.entry(e.session_id.clone()).or_insert(0) += 1;
    }
    let n = by_session.len();
    if n < MIN_SESSIONS_WITH_SIGNAL {
        return vec![];
    }
    let sum: f64 = by_session.values().map(|&c| c as f64).sum();
    let avg = sum / (n as f64);
    if avg < MIN_AVG_TRANSITIONS {
        return vec![];
    }
    vec![Bet {
        id: "H25:mode_thrash".into(),
        heuristic_id: "H25".into(),
        title: "Plan / agent mode thrash".into(),
        hypothesis: format!(
            "Average {:.1} mode transitions per session across {} sessions — prompt may be unstable.",
            avg, n
        ),
        expected_tokens_saved_per_week: sum * 40.0,
        effort_minutes: 20,
        evidence: vec![format!("Sessions with mode_transition: {}", n)],
        apply_step:
            "State desired mode up front; avoid toggling plan/agent mid-task unless blocked.".into(),
        evidence_recency_ms: inputs.window_end_ms,
        confidence: None,
        category: None,
    }]
}
