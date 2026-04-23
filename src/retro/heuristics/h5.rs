// SPDX-License-Identifier: AGPL-3.0-or-later
//! H5 — Idle session bloat (long Idle, not Done).

use crate::core::event::SessionStatus;
use crate::retro::types::{Bet, Inputs};
use std::collections::HashMap;

const IDLE_MS: u64 = 30 * 60 * 1000;

pub fn run(inputs: &Inputs) -> Vec<Bet> {
    let mut sessions: HashMap<String, &crate::core::event::SessionRecord> = HashMap::new();
    for (s, _) in &inputs.events {
        sessions.entry(s.id.clone()).or_insert(s);
    }
    let mut long_idle = 0u64;
    for s in sessions.values() {
        if s.status != SessionStatus::Idle {
            continue;
        }
        let end = s.ended_at_ms.unwrap_or(inputs.window_end_ms);
        if end.saturating_sub(s.started_at_ms) >= IDLE_MS {
            long_idle += 1;
        }
    }
    if long_idle < 2 {
        return vec![];
    }
    vec![Bet {
        id: "H5:idle-bloat".into(),
        heuristic_id: "H5".into(),
        title: "Long-lived Idle sessions".into(),
        hypothesis: format!(
            "{} sessions stayed Idle for 30+ minutes — inflates active session counts and sync noise.",
            long_idle
        ),
        expected_tokens_saved_per_week: (long_idle as f64) * 300.0,
        effort_minutes: 15,
        evidence: vec![format!("Idle ≥30m: {}", long_idle)],
        apply_step:
            "Tune idle TTL in agent settings or end sessions explicitly when stepping away.".into(),
        evidence_recency_ms: inputs.window_end_ms,
    }]
}
