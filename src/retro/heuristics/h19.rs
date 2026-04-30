// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::retro::types::{Bet, Inputs};
use std::collections::HashMap;

const PRESSURE_THRESHOLD: f64 = 0.8;
const MIN_SESSIONS: usize = 5;

pub fn run(inputs: &Inputs) -> Vec<Bet> {
    let sessions_at_pressure = sessions_over_pressure(inputs);
    if sessions_at_pressure < MIN_SESSIONS {
        return vec![];
    }
    vec![Bet {
        id: format!("H19:pressure:{sessions_at_pressure}"),
        heuristic_id: "H19".into(),
        title: format!("Context window near capacity in {sessions_at_pressure} sessions"),
        hypothesis: format!(
            "{sessions_at_pressure} sessions used ≥{:.0}% of context window. \
             Long sessions inflate costs and risk truncation.",
            PRESSURE_THRESHOLD * 100.0
        ),
        expected_tokens_saved_per_week: sessions_at_pressure as f64 * 2_000.0,
        effort_minutes: 30,
        evidence: vec![format!("{sessions_at_pressure} high-pressure sessions")],
        apply_step: "Split sessions earlier or prune CLAUDE.md / injected context.".into(),
        evidence_recency_ms: inputs.window_end_ms,
        confidence: None,
        category: None,
    }]
}

fn sessions_over_pressure(inputs: &Inputs) -> usize {
    let mut seen: HashMap<&str, bool> = HashMap::new();
    for (session, event) in &inputs.events {
        let (Some(used), Some(max)) = (event.context_used_tokens, event.context_max_tokens) else {
            continue;
        };
        if max == 0 {
            continue;
        }
        let ratio = used as f64 / max as f64;
        if ratio >= PRESSURE_THRESHOLD {
            seen.entry(session.id.as_str()).or_insert(true);
        }
    }
    seen.len()
}
