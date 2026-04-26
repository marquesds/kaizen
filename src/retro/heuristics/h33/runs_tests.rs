// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::core::event::{Event, EventKind, EventSource, SessionRecord, SessionStatus};
use crate::retro::heuristics::h33::runs::{run, runs_from_tool_seq};
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

fn tc(sid: &str, seq: u64, name: &str) -> (SessionRecord, Event) {
    (
        sess(sid),
        Event {
            session_id: sid.into(),
            seq,
            ts_ms: seq,
            ts_exact: true,
            kind: EventKind::ToolCall,
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

fn inputs(events: Vec<(SessionRecord, Event)>) -> Inputs {
    Inputs {
        window_start_ms: 0,
        window_end_ms: 100,
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
fn five_read_file_one_session_bet() {
    let mut ev = vec![];
    for i in 0..5 {
        ev.push(tc("s1", i, "read_file"));
    }
    let bets = run(&inputs(ev));
    assert!(bets.iter().any(|b| b.id == "H33:run:read_file"));
}

#[test]
fn four_read_file_no_bet() {
    let mut ev = vec![];
    for i in 0..4 {
        ev.push(tc("s1", i, "read_file"));
    }
    let bets = run(&inputs(ev));
    assert!(!bets.iter().any(|b| b.id == "H33:run:read_file"));
}

#[test]
fn two_sessions_same_tool_run() {
    let mut ev = vec![];
    for i in 0..5 {
        ev.push(tc("s1", i, "x"));
    }
    for i in 0..5 {
        ev.push(tc("s2", i + 10, "x"));
    }
    let bets = run(&inputs(ev));
    let b = bets.iter().find(|b| b.id == "H33:run:x").expect("bet");
    assert!(b.evidence.iter().any(|e| e.contains("s1")));
    assert!(b.evidence.iter().any(|e| e.contains("s2")));
}

#[test]
fn empty_inputs() {
    assert!(run(&inputs(vec![])).is_empty());
}

#[test]
fn runs_from_tool_seq_segments() {
    let t = ["a", "a", "b", "b", "b"];
    let r = runs_from_tool_seq(&t);
    assert_eq!(r, vec![("a".into(), 2), ("b".into(), 3)]);
}
