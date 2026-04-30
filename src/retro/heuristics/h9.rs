// SPDX-License-Identifier: AGPL-3.0-or-later
//! H9 — Error concentration: many `Error` events or errors spread across sessions.
//!
//! Provider-only remote inputs omit some indexes but still carry `error` kind events when synced.

use crate::core::event::EventKind;
use crate::retro::types::{Bet, Inputs};
use std::collections::HashSet;

/// Absolute floor: at least this many error rows in the window.
const MIN_ERROR_EVENTS: u64 = 6;
/// Need enough sessions before treating "share" as meaningful.
const MIN_SESSIONS: usize = 5;
/// Fire when at least this fraction of distinct sessions saw ≥1 error.
const SESSION_ERROR_SHARE: f64 = 0.22;

pub fn run(inputs: &Inputs) -> Vec<Bet> {
    let mut err_sessions: HashSet<String> = HashSet::new();
    let mut error_count = 0u64;
    for (_, e) in &inputs.events {
        if e.kind != EventKind::Error {
            continue;
        }
        error_count += 1;
        err_sessions.insert(e.session_id.clone());
    }
    let n_sessions = inputs.aggregates.unique_session_ids.len().max(1);
    let share = err_sessions.len() as f64 / n_sessions as f64;
    let share_ok =
        inputs.aggregates.unique_session_ids.len() >= MIN_SESSIONS && share >= SESSION_ERROR_SHARE;
    if error_count < MIN_ERROR_EVENTS && !share_ok {
        return vec![];
    }
    vec![Bet {
        id: "H9:errors".into(),
        heuristic_id: "H9".into(),
        title: "Elevated agent/tool error rate".into(),
        hypothesis: format!(
            "{} Error events across {} sessions ({:.0}% of {} sessions touched) — investigate flaky tools, proxy, or permissions.",
            error_count,
            err_sessions.len(),
            share * 100.0,
            n_sessions
        ),
        expected_tokens_saved_per_week: (error_count as f64) * 120.0,
        effort_minutes: 35,
        evidence: vec![
            format!("Error events: {}", error_count),
            format!("Sessions with ≥1 error: {}", err_sessions.len()),
        ],
        apply_step:
            "Check recent Error payloads in `kaizen sessions show` / logs; fix upstream tool or tighten retries."
                .into(),
        evidence_recency_ms: inputs.window_end_ms,
    confidence: None,
    category: None,
    }]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::event::{Event, EventSource, SessionRecord, SessionStatus};
    use crate::retro::types::RetroAggregates;
    use serde_json::json;
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
            parent_session_id: None,
            agent_version: None,
            os: None,
            arch: None,
            repo_file_count: None,
            repo_total_loc: None,
        }
    }

    #[test]
    fn fires_on_error_count() {
        let mut agg = RetroAggregates::default();
        for i in 0..6 {
            agg.unique_session_ids.insert(format!("s{i}"));
        }
        let events: Vec<_> = (0..6u64)
            .map(|i| {
                (
                    sess(&format!("s{i}")),
                    Event {
                        session_id: format!("s{i}"),
                        seq: i,
                        ts_ms: i,
                        ts_exact: true,
                        kind: EventKind::Error,
                        source: EventSource::Tail,
                        tool: None,
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
                        payload: json!({"upstream_error": "timeout"}),
                    },
                )
            })
            .collect();
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
            feedback: vec![],
            session_outcomes: vec![],
            session_sample_aggs: vec![],
        };
        let bets = run(&inputs);
        assert_eq!(bets.len(), 1);
        assert_eq!(bets[0].heuristic_id, "H9");
    }

    #[test]
    fn silent_when_few_errors() {
        let mut agg = RetroAggregates::default();
        agg.unique_session_ids.insert("s0".into());
        let inputs = Inputs {
            window_start_ms: 0,
            window_end_ms: 1000,
            events: vec![(
                sess("s0"),
                Event {
                    session_id: "s0".into(),
                    seq: 0,
                    ts_ms: 0,
                    ts_exact: true,
                    kind: EventKind::Error,
                    source: EventSource::Proxy,
                    tool: None,
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
                    payload: serde_json::Value::Null,
                },
            )],
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
            feedback: vec![],
            session_outcomes: vec![],
            session_sample_aggs: vec![],
        };
        assert!(run(&inputs).is_empty());
    }
}
