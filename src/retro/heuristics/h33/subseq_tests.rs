// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::core::event::{Event, EventKind, EventSource, SessionRecord, SessionStatus};
use crate::retro::heuristics::h33::subseq::{find_repeating_subseqs, run};
use crate::retro::types::{Inputs, RetroAggregates};
use std::collections::HashSet;

#[test]
fn grep_read_triple() {
    let t = ["grep", "read", "grep", "read", "grep", "read"];
    let r = find_repeating_subseqs(&t, 2);
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].0, vec!["grep".to_string(), "read".to_string()]);
    assert_eq!(r[0].1, 3);
}

#[test]
fn grep_read_grep_no_bet_len2() {
    let t = ["grep", "read", "grep"];
    assert!(find_repeating_subseqs(&t, 2).is_empty());
}

#[test]
fn len3_twice() {
    let t = ["a", "b", "c", "a", "b", "c"];
    let r = find_repeating_subseqs(&t, 3);
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].1, 2);
}

#[test]
fn pure_helper_abab_two_only() {
    let t = ["a", "b", "a", "b"];
    assert!(find_repeating_subseqs(&t, 2).is_empty());
}

#[test]
fn subseq_bet_from_inputs() {
    let sess = SessionRecord {
        id: "s1".into(),
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
    };
    let mut ev = vec![];
    let names = ["grep", "read", "grep", "read", "grep", "read"];
    for (i, n) in names.iter().enumerate() {
        ev.push((
            sess.clone(),
            Event {
                session_id: "s1".into(),
                seq: i as u64,
                ts_ms: i as u64,
                ts_exact: true,
                kind: EventKind::ToolCall,
                source: EventSource::Tail,
                tool: Some((*n).into()),
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
        ));
    }
    let inputs = Inputs {
        window_start_ms: 0,
        window_end_ms: 100,
        events: ev,
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
    };
    let bets = run(&inputs);
    assert!(bets.iter().any(|b| b.id == "H33:subseq:grep+read"));
}
