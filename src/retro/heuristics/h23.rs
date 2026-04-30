// SPDX-License-Identifier: AGPL-3.0-or-later
//! H23 — Todo abandonment: many open todos, low completion share.

use crate::core::event::EventKind;
use crate::retro::types::{Bet, Inputs};
use std::collections::HashSet;

const MIN_TODOS: u64 = 5;
const MAX_COMPLETION_SHARE: f64 = 0.4;
const MIN_BAD_SESSIONS: usize = 3;

pub fn run(inputs: &Inputs) -> Vec<Bet> {
    let mut bad = HashSet::new();
    for (s, e) in &inputs.events {
        if e.kind != EventKind::Lifecycle {
            continue;
        }
        if e.payload.get("type").and_then(|t| t.as_str()) != Some("todo_write") {
            continue;
        }
        let total = e
            .payload
            .get("todos_total")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let comp = e
            .payload
            .get("todos_completed")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        if total < MIN_TODOS {
            continue;
        }
        let rate = (comp as f64) / (total as f64);
        if rate < MAX_COMPLETION_SHARE {
            bad.insert(s.id.clone());
        }
    }
    if bad.len() < MIN_BAD_SESSIONS {
        return vec![];
    }
    vec![Bet {
        id: "H23:todo_abandon".into(),
        heuristic_id: "H23".into(),
        title: "Todo lists abandoned mid-session".into(),
        hypothesis: format!(
            "{} sessions show TodoWrite snapshots with ≥{} items but <{:.0}% completed — scope may be too wide.",
            bad.len(),
            MIN_TODOS,
            MAX_COMPLETION_SHARE * 100.0
        ),
        expected_tokens_saved_per_week: (bad.len() as f64) * 120.0,
        effort_minutes: 25,
        evidence: bad.iter().take(8).cloned().collect(),
        apply_step:
            "Narrow task scope; skip TodoWrite for small fixes; close or cancel stale items.".into(),
        evidence_recency_ms: inputs.window_end_ms,
        confidence: None,
        category: None,
    }]
}
