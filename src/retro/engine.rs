// SPDX-License-Identifier: AGPL-3.0-or-later
//! Merge heuristics, dedupe vs prior reports, rank top bets.

use crate::retro::heuristics;
use crate::retro::types::{Bet, BetCategory, Confidence, Inputs, Report, RetroMeta, RetroStats};
use std::collections::{HashMap, HashSet};

const TOP_BET_N: usize = 1;
const INVESTIGATE_N: usize = 2;
const HYGIENE_N: usize = 2;

/// Pure ranking step after `Inputs` are assembled.
pub fn run(inputs: &Inputs, prior_bet_ids: &HashSet<String>) -> Report {
    let mut candidates = heuristics::all_bets(inputs);
    candidates.iter_mut().for_each(enrich_bet);
    candidates.sort_by(|a, b| {
        b.score()
            .partial_cmp(&a.score())
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.evidence_recency_ms.cmp(&a.evidence_recency_ms))
            .then_with(|| a.id.cmp(&b.id))
    });

    let (available, skipped) = available_candidates(candidates, prior_bet_ids);
    let top = select_grouped_bets(&available);

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

fn enrich_bet(bet: &mut Bet) {
    let (confidence, category) = heuristic_metadata(&bet.heuristic_id);
    bet.confidence = Some(confidence);
    bet.category = Some(category);
}

fn heuristic_metadata(heuristic_id: &str) -> (Confidence, BetCategory) {
    match heuristic_id {
        "H1" | "H29" => (Confidence::High, BetCategory::QuickWin),
        "H9" | "H10" | "H12" | "H19" | "H21" | "H27" | "H33" => {
            (Confidence::High, BetCategory::Investigation)
        }
        "H2" | "H3" | "H11" | "H14" | "H22" | "H24" => {
            (Confidence::Medium, BetCategory::Investigation)
        }
        "H7" | "H20" | "H28" | "H30" | "H31" | "H32" => (Confidence::Medium, BetCategory::Hygiene),
        _ => (Confidence::Low, BetCategory::Hygiene),
    }
}

fn available_candidates(
    candidates: Vec<Bet>,
    prior_bet_ids: &HashSet<String>,
) -> (Vec<Bet>, Vec<String>) {
    let mut skipped = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut available = Vec::new();
    for bet in candidates {
        if prior_bet_ids.contains(&bet.id) {
            skipped.push(format!("{} ({})", bet.title, bet.id));
        } else if seen.insert(bet.id.clone()) {
            available.push(bet);
        }
    }
    (available, skipped)
}

fn select_grouped_bets(candidates: &[Bet]) -> Vec<Bet> {
    let mut selected_ids: HashSet<String> = HashSet::new();
    let mut top = Vec::new();
    push_matches(&mut top, &mut selected_ids, candidates, TOP_BET_N, |b| {
        b.confidence == Some(Confidence::High)
    });
    push_matches(
        &mut top,
        &mut selected_ids,
        candidates,
        INVESTIGATE_N,
        |b| {
            b.category == Some(BetCategory::Investigation)
                && matches!(b.confidence, Some(Confidence::High | Confidence::Medium))
        },
    );
    push_matches(&mut top, &mut selected_ids, candidates, HYGIENE_N, |b| {
        matches!(
            b.category,
            Some(BetCategory::QuickWin | BetCategory::Hygiene)
        )
    });
    top
}

fn push_matches<F>(
    out: &mut Vec<Bet>,
    selected_ids: &mut HashSet<String>,
    candidates: &[Bet],
    limit: usize,
    mut pred: F,
) where
    F: FnMut(&Bet) -> bool,
{
    let mut added = 0;
    for bet in candidates {
        if added == limit {
            break;
        }
        if pred(bet) && selected_ids.insert(bet.id.clone()) {
            out.push(bet.clone());
            added += 1;
        }
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
    use crate::retro::types::{Bet, RetroAggregates, SkillFileOnDisk};
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
                    prompt_fingerprint: None,
                    parent_session_id: None,
                    agent_version: None,
                    os: None,
                    arch: None,
                    repo_file_count: None,
                    repo_total_loc: None,
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
                    stop_reason: None,
                    latency_ms: None,
                    ttft_ms: None,
                    retry_count: None,
                    context_used_tokens: None,
                    context_max_tokens: None,
                    cache_creation_tokens: None,
                    cache_read_tokens: None,
                    system_prompt_tokens: None,
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
            prompt_fingerprints: vec![],
            feedback: vec![],
            session_outcomes: vec![],
            session_sample_aggs: vec![],
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

    #[test]
    fn metadata_is_added_to_bets() {
        let inputs = minimal_inputs();
        let r = run(&inputs, &HashSet::new());
        assert!(r.top_bets.iter().all(|b| b.confidence.is_some()));
        assert!(r.top_bets.iter().all(|b| b.category.is_some()));
    }

    #[test]
    fn selection_uses_one_two_two_shape() {
        let mut bets = vec![
            bet("H1:a", "H1", 1000.0),
            bet("H9:a", "H9", 900.0),
            bet("H2:a", "H2", 800.0),
            bet("H7:a", "H7", 700.0),
            bet("H4:a", "H4", 600.0),
        ];
        bets.iter_mut().for_each(enrich_bet);
        bets.sort_by(|a, b| b.score().partial_cmp(&a.score()).unwrap());
        let top = select_grouped_bets(&bets);
        assert_eq!(
            top.iter().map(|b| b.id.as_str()).collect::<Vec<_>>(),
            vec!["H1:a", "H9:a", "H2:a", "H7:a", "H4:a"]
        );
    }

    fn bet(id: &str, heuristic_id: &str, tokens: f64) -> Bet {
        Bet {
            id: id.into(),
            heuristic_id: heuristic_id.into(),
            title: id.into(),
            hypothesis: String::new(),
            expected_tokens_saved_per_week: tokens,
            effort_minutes: 10,
            evidence: vec![],
            apply_step: String::new(),
            evidence_recency_ms: 0,
            confidence: None,
            category: None,
        }
    }
}
