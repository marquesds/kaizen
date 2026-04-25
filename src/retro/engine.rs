// SPDX-License-Identifier: AGPL-3.0-or-later
//! Merge heuristics, dedupe vs prior reports, rank top bets.

use crate::retro::heuristics;
use crate::retro::types::{Inputs, Report, RetroMeta, RetroStats};
use std::collections::{HashMap, HashSet};

const TOP_N: usize = 5;

/// Pure ranking step after `Inputs` are assembled.
pub fn run(inputs: &Inputs, prior_bet_ids: &HashSet<String>) -> Report {
    let mut candidates = heuristics::all_bets(inputs);
    candidates.sort_by(|a, b| {
        b.score()
            .partial_cmp(&a.score())
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.evidence_recency_ms.cmp(&a.evidence_recency_ms))
            .then_with(|| a.id.cmp(&b.id))
    });

    let mut skipped = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut top = Vec::new();
    for bet in candidates {
        if prior_bet_ids.contains(&bet.id) {
            skipped.push(format!("{} ({})", bet.title, bet.id));
            continue;
        }
        if !seen.insert(bet.id.clone()) {
            continue;
        }
        if top.len() < TOP_N {
            top.push(bet);
        }
    }

    let session_count = inputs.aggregates.unique_session_ids.len() as u64;
    let (top_model, top_model_pct) = top_model_share(&inputs.aggregates.model_session_counts);
    let (top_tool, top_tool_pct) = top_tool_share(&inputs.aggregates.tool_event_counts);
    let median_min = median_session_minutes(inputs);

    Report {
        meta: RetroMeta {
            week_label: String::new(),
            span_start_ms: inputs.window_start_ms,
            span_end_ms: inputs.window_end_ms,
            session_count,
            total_cost_usd_e6: inputs.aggregates.total_cost_usd_e6,
        },
        top_bets: top,
        skipped_deduped: skipped,
        stats: RetroStats {
            sessions: session_count,
            total_cost_usd_e6: inputs.aggregates.total_cost_usd_e6,
            top_model,
            top_model_pct,
            top_tool,
            top_tool_pct,
            median_session_minutes: median_min,
        },
    }
}

fn top_model_share(m: &HashMap<String, u64>) -> (Option<String>, Option<u64>) {
    let total: u64 = m.values().sum();
    if total == 0 {
        return (None, None);
    }
    let (k, v) = m.iter().max_by_key(|(_, c)| *c).unwrap();
    let pct = (*v * 100) / total;
    (Some(k.clone()), Some(pct))
}

fn top_tool_share(m: &HashMap<String, u64>) -> (Option<String>, Option<u64>) {
    let total: u64 = m.values().sum();
    if total == 0 {
        return (None, None);
    }
    let (k, v) = m.iter().max_by_key(|(_, c)| *c).unwrap();
    let pct = (*v * 100) / total;
    (Some(k.clone()), Some(pct))
}

fn median_session_minutes(inputs: &Inputs) -> Option<u64> {
    let mut by_id: HashMap<String, (u64, Option<u64>)> = HashMap::new();
    for (s, _) in &inputs.events {
        by_id
            .entry(s.id.clone())
            .or_insert((s.started_at_ms, s.ended_at_ms));
    }
    let mut durations: Vec<u64> = by_id
        .into_values()
        .map(|(start, end)| {
            let e = end.unwrap_or(inputs.window_end_ms);
            e.saturating_sub(start) / 60_000
        })
        .collect();
    if durations.is_empty() {
        return None;
    }
    durations.sort_unstable();
    Some(durations[durations.len() / 2])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::event::{Event, EventKind, EventSource, SessionRecord, SessionStatus};
    use crate::retro::types::{RetroAggregates, SkillFileOnDisk};
    use serde_json::json;
    use std::collections::HashSet;

    fn minimal_inputs() -> Inputs {
        let mut agg = RetroAggregates::default();
        agg.unique_session_ids.insert("s1".into());
        agg.tool_event_counts.insert("read_file".into(), 20);
        agg.tool_event_counts.insert("x".into(), 2);
        Inputs {
            window_start_ms: 0,
            window_end_ms: 1000,
            events: vec![(
                SessionRecord {
                    id: "s1".into(),
                    agent: "cursor".into(),
                    model: Some("m".into()),
                    workspace: "/w".into(),
                    started_at_ms: 0,
                    ended_at_ms: Some(120_000),
                    status: SessionStatus::Done,
                    trace_path: "".into(),
                    start_commit: None,
                    end_commit: None,
                    branch: None,
                    dirty_start: None,
                    dirty_end: None,
                    repo_binding_source: None,
                },
                Event {
                    session_id: "s1".into(),
                    seq: 0,
                    ts_ms: 100,
                    ts_exact: false,
                    kind: EventKind::ToolCall,
                    source: EventSource::Tail,
                    tool: Some("read_file".into()),
                    tool_call_id: None,
                    tokens_in: None,
                    tokens_out: None,
                    reasoning_tokens: None,
                    cost_usd_e6: None,
                    payload: json!({}),
                },
            )],
            files_touched: vec![],
            skills_used: vec![],
            tool_spans: vec![],
            skills_used_recent_slugs: HashSet::new(),
            usage_lookback_ms: 0,
            skill_files_on_disk: vec![SkillFileOnDisk {
                slug: "z".into(),
                size_bytes: 100,
                mtime_ms: 0,
            }],
            rule_files_on_disk: vec![],
            rules_used_recent_slugs: HashSet::new(),
            file_facts: HashMap::new(),
            eval_scores: vec![],
            aggregates: agg,
        }
    }

    #[test]
    fn dedupes_prior_ids() {
        let inputs = minimal_inputs();
        let mut prior = HashSet::new();
        prior.insert("H4:read_file".into());
        let r = run(&inputs, &prior);
        assert!(r.top_bets.iter().all(|b| b.id != "H4:read_file"));
        assert!(!r.skipped_deduped.is_empty() || r.top_bets.len() <= 4);
    }
}
