// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::retro::types::{Bet, Inputs};

const MAX_DEPTH_TRIGGER: u32 = 4;
const MAX_FAN_OUT_TRIGGER: u32 = 8;
const TOKENS_PER_DEPTH: f64 = 600.0;

pub fn run(inputs: &Inputs) -> Vec<Bet> {
    let Some(ref stats) = inputs.aggregates.span_tree_stats else {
        return vec![];
    };
    if stats.max_depth < MAX_DEPTH_TRIGGER && stats.max_fan_out < MAX_FAN_OUT_TRIGGER {
        return vec![];
    }
    let deepest = inputs
        .tool_spans
        .iter()
        .find(|s| s.span_id == stats.deepest_span_id);
    let tool_label = deepest.map(|s| s.tool.as_str()).unwrap_or("unknown");
    let subtree_cost = deepest.and_then(|s| s.subtree_cost_usd_e6).unwrap_or(0);
    let savings = subtree_cost as f64 / 1_000.0 + stats.max_depth as f64 * TOKENS_PER_DEPTH;
    vec![Bet {
        id: format!("H18:depth{}:{}", stats.max_depth, tool_label),
        heuristic_id: "H18".into(),
        title: format!(
            "Agent looping inside tool chain — {} at depth {} with {} children",
            tool_label, stats.max_depth, stats.max_fan_out
        ),
        hypothesis: format!(
            "Span tree depth {}, fan-out {}. Nested tool calls inflate context unnecessarily.",
            stats.max_depth, stats.max_fan_out
        ),
        expected_tokens_saved_per_week: savings,
        effort_minutes: 45,
        evidence: vec![stats.deepest_span_id.clone()],
        apply_step: "Flatten the tool call chain; avoid calling tools from within tool handlers."
            .into(),
        evidence_recency_ms: inputs.window_end_ms,
        confidence: None,
        category: None,
    }]
}
