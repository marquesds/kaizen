// SPDX-License-Identifier: AGPL-3.0-or-later
//! H24 — High reject_diff rate on edit-like work (hooks + tool calls).

use crate::core::event::EventKind;
use crate::retro::types::{Bet, Inputs};

const MIN_ATTEMPTS: u64 = 8;
const REJECT_SHARE_TRIGGER: f64 = 0.15;

fn is_edit_tool(name: &str) -> bool {
    let n = name.to_lowercase();
    n.contains("edit")
        || n.contains("write")
        || n.contains("apply")
        || n.contains("patch")
        || n.contains("search_replace")
}

pub fn run(inputs: &Inputs) -> Vec<Bet> {
    let mut rejects = 0u64;
    let mut edits = 0u64;
    for (_, e) in &inputs.events {
        if e.kind == EventKind::Hook
            && e.payload.get("reject_diff").and_then(|v| v.as_bool()) == Some(true)
        {
            rejects += 1;
        }
        if e.kind == EventKind::ToolCall && e.tool.as_deref().is_some_and(is_edit_tool) {
            edits += 1;
        }
    }
    if edits < MIN_ATTEMPTS {
        return vec![];
    }
    let share = (rejects as f64) / (edits as f64);
    if share < REJECT_SHARE_TRIGGER {
        return vec![];
    }
    vec![Bet {
        id: "H24:reject_diff".into(),
        heuristic_id: "H24".into(),
        title: "Frequent diff rejections".into(),
        hypothesis: format!(
            "{:.0}% of edit-like tool calls ({}/{}) pair with reject_diff hooks — drafts may be too speculative.",
            share * 100.0,
            rejects,
            edits
        ),
        expected_tokens_saved_per_week: (rejects as f64) * 90.0,
        effort_minutes: 30,
        evidence: vec![
            format!("reject_diff hooks: {}", rejects),
            format!("edit-like tools: {}", edits),
        ],
        apply_step:
            "Tighten AGENTS.md / rules; plan before multi-file edits; smaller diffs per turn."
                .into(),
        evidence_recency_ms: inputs.window_end_ms,
        confidence: None,
        category: None,
    }]
}
