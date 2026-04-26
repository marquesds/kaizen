// SPDX-License-Identifier: AGPL-3.0-or-later
//! H33 — Automation opportunities from repeated tool calls (runs + subsequences).

pub(crate) mod runs;
pub(crate) mod subseq;

#[cfg(test)]
mod runs_tests;

#[cfg(test)]
mod subseq_tests;

use crate::retro::types::{Bet, Inputs};

pub fn run(inputs: &Inputs) -> Vec<Bet> {
    let mut v = runs::run(inputs);
    v.extend(subseq::run(inputs));
    v
}

#[cfg(test)]
mod tests {
    use super::run;
    use crate::core::event::{Event, EventSource, SessionRecord, SessionStatus};
    use crate::retro::types::{Inputs, RetroAggregates};
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
            trace_path: "/t".into(),
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

    fn tool_call(sid: &str, seq: u64, name: &str) -> (SessionRecord, Event) {
        (
            sess(sid),
            Event {
                session_id: sid.into(),
                seq,
                ts_ms: seq,
                ts_exact: true,
                kind: crate::core::event::EventKind::ToolCall,
                source: EventSource::Tail,
                tool: Some(name.into()),
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
        )
    }

    fn base_inputs(events: Vec<(SessionRecord, Event)>) -> Inputs {
        Inputs {
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
            aggregates: RetroAggregates::default(),
            prompt_fingerprints: vec![],
            feedback: vec![],
            session_outcomes: vec![],
            session_sample_aggs: vec![],
        }
    }

    #[test]
    fn smoke_merged_bets() {
        let mut ev = vec![];
        for i in 0..5 {
            ev.push(tool_call("a", i, "read_file"));
        }
        ev.push(tool_call("a", 10, "grep"));
        ev.push(tool_call("a", 11, "read"));
        ev.push(tool_call("a", 12, "grep"));
        ev.push(tool_call("a", 13, "read"));
        ev.push(tool_call("a", 14, "grep"));
        ev.push(tool_call("a", 15, "read"));
        let inputs = base_inputs(ev);
        let bets = run(&inputs);
        assert!(bets.iter().any(|b| b.id.starts_with("H33:run:")));
        assert!(bets.iter().any(|b| b.id.starts_with("H33:subseq:")));
    }
}
