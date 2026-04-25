// SPDX-License-Identifier: AGPL-3.0-or-later
//! H11 — Session cost outlier: one session drives a disproportionate share of attributed spend.

use crate::retro::types::{Bet, Inputs};
use std::collections::HashMap;

const MIN_SESSIONS: usize = 6;
/// Outlier if max session cost ≥ this multiple of the per-session mean.
const OUTLIER_VS_MEAN: f64 = 4.0;
/// Ignore noise when the hottest session is still tiny (micro-USD).
const MIN_MAX_COST_E6: i64 = 40_000;

pub fn run(inputs: &Inputs) -> Vec<Bet> {
    let n = inputs.aggregates.unique_session_ids.len();
    if n < MIN_SESSIONS {
        return vec![];
    }
    let mut by_session: HashMap<String, i64> = HashMap::new();
    for (_, e) in &inputs.events {
        if let Some(c) = e.cost_usd_e6 {
            if c <= 0 {
                continue;
            }
            *by_session.entry(e.session_id.clone()).or_default() += c;
        }
    }
    let Some((max_sid, max_cost)) = by_session.iter().max_by_key(|(_, c)| *c) else {
        return vec![];
    };
    if *max_cost < MIN_MAX_COST_E6 {
        return vec![];
    }
    let total: i64 = by_session.values().sum();
    let mean = (total as f64) / (n as f64);
    if mean <= 0.0 {
        return vec![];
    }
    if (*max_cost as f64) < mean * OUTLIER_VS_MEAN {
        return vec![];
    }

    vec![Bet {
        id: format!("H11:{max_sid}"),
        heuristic_id: "H11".into(),
        title: "One session dominates cost".into(),
        hypothesis: format!(
            "Session `{}` accounts for ~${:.2} while the per-session mean is ~${:.2} — inspect long runs, premium model use, or runaway tool loops.",
            max_sid,
            (*max_cost as f64) / 1_000_000.0,
            mean / 1_000_000.0
        ),
        expected_tokens_saved_per_week: (*max_cost as f64) / 5000.0,
        effort_minutes: 30,
        evidence: vec![
            format!("Max session cost (e6 USD): {}", max_cost),
            format!("Sessions in window: {}", n),
        ],
        apply_step:
            "Open the hottest session transcript; trim tool fan-out, shorten prompts, or downgrade model for mechanical steps."
                .into(),
        evidence_recency_ms: inputs.window_end_ms,
    }]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::event::{Event, EventKind, EventSource, SessionRecord, SessionStatus};
    use crate::retro::types::RetroAggregates;
    use std::collections::HashSet;

    fn sess(id: &str) -> SessionRecord {
        SessionRecord {
            id: id.into(),
            agent: "cursor".into(),
            model: None,
            workspace: "/w".into(),
            started_at_ms: 0,
            ended_at_ms: None,
            status: SessionStatus::Done,
            trace_path: String::new(),
            start_commit: None,
            end_commit: None,
            branch: None,
            dirty_start: None,
            dirty_end: None,
            repo_binding_source: None,
            prompt_fingerprint: None,
        }
    }

    #[test]
    fn fires_on_cost_spike() {
        let mut agg = RetroAggregates::default();
        let mut events = vec![];
        for i in 0..6 {
            let sid = format!("s{i}");
            agg.unique_session_ids.insert(sid.clone());
            let cost = if i == 0 { 500_000i64 } else { 10_000i64 };
            events.push((
                sess(&sid),
                Event {
                    session_id: sid,
                    seq: 0,
                    ts_ms: i as u64,
                    ts_exact: true,
                    kind: EventKind::Cost,
                    source: EventSource::Proxy,
                    tool: None,
                    tool_call_id: None,
                    tokens_in: None,
                    tokens_out: None,
                    reasoning_tokens: None,
                    cost_usd_e6: Some(cost),
                    payload: serde_json::Value::Null,
                },
            ));
        }
        let inputs = Inputs {
            window_start_ms: 0,
            window_end_ms: 1000,
            events,
            files_touched: vec![],
            skills_used: vec![],
            tool_spans: vec![],
            skills_used_recent_slugs: HashSet::new(),
            usage_lookback_ms: 0,
            skill_files_on_disk: vec![],
            rule_files_on_disk: vec![],
            rules_used_recent_slugs: HashSet::new(),
            file_facts: Default::default(),
            eval_scores: vec![],
            aggregates: agg,
            prompt_fingerprints: vec![],
        };
        let bets = run(&inputs);
        assert_eq!(bets.len(), 1);
        assert_eq!(bets[0].heuristic_id, "H11");
    }
}
