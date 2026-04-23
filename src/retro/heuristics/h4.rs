//! H4 — High-cost tool concentration.

use crate::retro::types::{Bet, Inputs};

const TOP_SHARE: f64 = 0.25;
const MIN_EVENTS: u64 = 15;

pub fn run(inputs: &Inputs) -> Vec<Bet> {
    let total_events: u64 = inputs.aggregates.tool_event_counts.values().sum();
    if total_events < MIN_EVENTS {
        return vec![];
    }
    let mut pairs: Vec<(String, u64, i64)> = inputs
        .aggregates
        .tool_event_counts
        .iter()
        .map(|(t, c)| {
            let cost = inputs
                .aggregates
                .tool_cost_usd_e6
                .get(t)
                .copied()
                .unwrap_or(0);
            (t.clone(), *c, cost)
        })
        .collect();
    pairs.sort_by_key(|p| std::cmp::Reverse(p.1));
    let Some((tool, count, cost_e6)) = pairs.first() else {
        return vec![];
    };
    let share = (*count as f64) / (total_events as f64);
    if share < TOP_SHARE {
        return vec![];
    }
    let est_tokens = (*count as f64) * 200.0 + (*cost_e6 as f64 / 10_000.0);
    let id = format!("H4:{tool}");
    vec![Bet {
        id,
        heuristic_id: "H4".into(),
        title: format!("Tool `{}` dominates agent traffic", tool),
        hypothesis: format!(
            "`{}` accounts for {:.0}% of tool calls in the window — tighten rules or add cheaper shortcuts.",
            tool,
            share * 100.0
        ),
        expected_tokens_saved_per_week: est_tokens,
        effort_minutes: 30,
        evidence: vec![
            format!("Calls: {} / {} total.", count, total_events),
            format!("Attributed cost (micro-USD sum): {}", cost_e6),
        ],
        apply_step: format!(
            "Review read/search patterns involving `{}`; add project-specific rules or smaller entrypoint files.",
            tool
        ),
        evidence_recency_ms: inputs.window_end_ms,
    }]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retro::types::RetroAggregates;
    use std::collections::HashSet;

    #[test]
    fn flags_dominant_tool() {
        let mut agg = RetroAggregates::default();
        agg.tool_event_counts.insert("read_file".into(), 20);
        agg.tool_event_counts.insert("grep".into(), 3);
        let inputs = Inputs {
            window_start_ms: 0,
            window_end_ms: 1,
            events: vec![],
            files_touched: vec![],
            skills_used: vec![],
            skills_used_recent_slugs: HashSet::new(),
            usage_lookback_ms: 0,
            skill_files_on_disk: vec![],
            aggregates: agg,
        };
        let bets = run(&inputs);
        assert_eq!(bets.len(), 1);
        assert!(bets[0].title.contains("read_file"));
    }
}
