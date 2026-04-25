// SPDX-License-Identifier: AGPL-3.0-or-later
//! H13 — Delegation load: MCP-heavy tool mix and/or Cursor subagent sessions.
//!
//! Subagent detection uses `SessionRecord.trace_path` (local Cursor transcripts); remote-only
//! rows often omit this path, so the subagent bet may not fire for provider-only sources.

use crate::core::event::EventKind;
use crate::retro::types::{Bet, Inputs};

const MIN_TOOL_CALLS: u64 = 20;
const MIN_MCP_SHARE: f64 = 0.12;

const MIN_SESSIONS_TOTAL: usize = 6;
const MIN_SUBAGENT_SESSION_SHARE: f64 = 0.15;

pub fn run(inputs: &Inputs) -> Vec<Bet> {
    let mut out = Vec::new();
    let mut tool_calls = 0u64;
    let mut mcp_calls = 0u64;
    for (_, e) in &inputs.events {
        if e.kind != EventKind::ToolCall {
            continue;
        }
        tool_calls += 1;
        if e.tool
            .as_ref()
            .is_some_and(|t| t.to_lowercase().contains("mcp"))
        {
            mcp_calls += 1;
        }
    }
    if tool_calls >= MIN_TOOL_CALLS {
        let share = (mcp_calls as f64) / (tool_calls as f64);
        if share >= MIN_MCP_SHARE {
            out.push(Bet {
                id: "H13:mcp".into(),
                heuristic_id: "H13".into(),
                title: "High MCP tool share".into(),
                hypothesis: format!(
                    "{:.0}% of tool calls ({}/{}) reference MCP — each round-trip adds latency and context.",
                    share * 100.0,
                    mcp_calls,
                    tool_calls
                ),
                expected_tokens_saved_per_week: (mcp_calls as f64) * 80.0,
                effort_minutes: 30,
                evidence: vec![
                    format!("MCP-like tool calls: {}", mcp_calls),
                    format!("Total tool calls: {}", tool_calls),
                ],
                apply_step:
                    "Cache MCP results in-repo, narrow tool allowlists, or batch reads instead of chatty MCP loops."
                        .into(),
                evidence_recency_ms: inputs.window_end_ms,
            });
        }
    }

    let n_sess = inputs.aggregates.unique_session_ids.len();
    if n_sess >= MIN_SESSIONS_TOTAL {
        let mut seen = std::collections::HashSet::new();
        for (s, _) in &inputs.events {
            if s.trace_path.to_lowercase().contains("subagents") {
                seen.insert(s.id.clone());
            }
        }
        let sub_n = seen.len();
        let share = (sub_n as f64) / (n_sess as f64);
        if share >= MIN_SUBAGENT_SESSION_SHARE {
            out.push(Bet {
                id: "H13:subagents".into(),
                heuristic_id: "H13".into(),
                title: "Many Cursor subagent sessions".into(),
                hypothesis: format!(
                    "{} of {} sessions ({:.0}%) trace to `subagents/` transcripts — fan-out may inflate cost.",
                    sub_n,
                    n_sess,
                    share * 100.0
                ),
                expected_tokens_saved_per_week: (sub_n as f64) * 250.0,
                effort_minutes: 35,
                evidence: vec![format!("Subagent-tagged sessions: {}", sub_n)],
                apply_step:
                    "Prefer single-session workflows where possible; cap subagent depth or narrow delegated prompts."
                        .into(),
                evidence_recency_ms: inputs.window_end_ms,
            });
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::event::{Event, EventSource, SessionRecord, SessionStatus};
    use crate::retro::types::RetroAggregates;
    use std::collections::HashSet;

    fn sess(id: &str, trace: &str) -> SessionRecord {
        SessionRecord {
            id: id.into(),
            agent: "cursor".into(),
            model: None,
            workspace: "/w".into(),
            started_at_ms: 0,
            ended_at_ms: None,
            status: SessionStatus::Done,
            trace_path: trace.into(),
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
    fn mcp_share_bet() {
        let mut agg = RetroAggregates::default();
        let mut events = vec![];
        for i in 0..25 {
            let sid = format!("s{}", i % 5);
            agg.unique_session_ids.insert(sid.clone());
            events.push((
                sess(&sid, "/tmp/x"),
                Event {
                    session_id: sid,
                    seq: i as u64,
                    ts_ms: i as u64,
                    ts_exact: true,
                    kind: EventKind::ToolCall,
                    source: EventSource::Tail,
                    tool: Some(if i % 3 == 0 {
                        "mcp_github_search".into()
                    } else {
                        "read_file".into()
                    }),
                    tool_call_id: None,
                    tokens_in: None,
                    tokens_out: None,
                    reasoning_tokens: None,
                    cost_usd_e6: None,
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
        assert!(bets.iter().any(|b| b.id == "H13:mcp"));
    }

    #[test]
    fn subagent_share_bet() {
        let mut agg = RetroAggregates::default();
        let mut events = vec![];
        for i in 0..6 {
            let trace = if i < 2 {
                "/proj/agent-transcripts/abc/subagents/foo.jsonl"
            } else {
                "/proj/agent-transcripts/abc/agent.jsonl"
            };
            let sid = format!("s{i}");
            agg.unique_session_ids.insert(sid.clone());
            events.push((
                sess(&sid, trace),
                Event {
                    session_id: sid,
                    seq: 0,
                    ts_ms: i as u64,
                    ts_exact: true,
                    kind: EventKind::ToolCall,
                    source: EventSource::Tail,
                    tool: Some("grep".into()),
                    tool_call_id: None,
                    tokens_in: None,
                    tokens_out: None,
                    reasoning_tokens: None,
                    cost_usd_e6: None,
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
        assert!(bets.iter().any(|b| b.id == "H13:subagents"));
    }
}
