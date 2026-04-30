// SPDX-License-Identifier: AGPL-3.0-or-later
//! H10 — Shell / test failure proxy: repeated failing terminal tool results in one session.

use crate::core::event::EventKind;
use crate::retro::types::{Bet, Inputs};
use serde_json::Value;
use std::collections::HashMap;

/// Failing tool-result rows needed in a single session before we emit a bet.
const MIN_FAILING_RESULTS: u32 = 3;

fn shell_like_tool(name: &str) -> bool {
    let n = name.to_lowercase();
    n.contains("bash")
        || n.contains("shell")
        || n.contains("powershell")
        || n.contains("zsh")
        || n.contains("terminal")
        || n.contains("run_terminal")
        || n.contains("execute_command")
        || n == "exec"
        || n == "sh"
}

fn tool_result_body_text(payload: &Value) -> String {
    if let Some(arr) = payload.get("content").and_then(|c| c.as_array()) {
        let mut s = String::new();
        for block in arr {
            if let Some(t) = block.get("text").and_then(|x| x.as_str()) {
                s.push_str(t);
                s.push('\n');
            }
        }
        if !s.is_empty() {
            return s;
        }
    }
    payload.to_string()
}

fn tool_result_looks_failed(payload: &Value, text: &str) -> bool {
    if payload.get("is_error").and_then(|v| v.as_bool()) == Some(true) {
        return true;
    }
    let low = text.to_lowercase();
    if low.contains("non-zero exit") {
        return true;
    }
    if low.contains("exit code 1")
        || low.contains("exit code: 1")
        || low.contains("exit code:1")
        || low.contains("exit status 1")
        || low.contains("exit status: 1")
    {
        return true;
    }
    if low.contains("command exited with code") && !low.contains("code 0") {
        return true;
    }
    if low.contains("tests failed") || low.contains("test failed") || low.contains("failures:") {
        return true;
    }
    if low.contains("error:") && (low.contains("failed") || low.contains("command")) {
        return true;
    }
    false
}

pub fn run(inputs: &Inputs) -> Vec<Bet> {
    let mut by_session: HashMap<String, u32> = HashMap::new();
    let mut id_to_tool: HashMap<String, String> = HashMap::new();

    for (_, e) in &inputs.events {
        match e.kind {
            EventKind::ToolCall => {
                if let (Some(id), Some(tool)) = (&e.tool_call_id, &e.tool) {
                    id_to_tool.insert(id.clone(), tool.clone());
                }
            }
            EventKind::ToolResult => {
                let Some(tid) = &e.tool_call_id else {
                    continue;
                };
                let Some(tool_name) = id_to_tool.get(tid) else {
                    continue;
                };
                if !shell_like_tool(tool_name) {
                    continue;
                }
                let text = tool_result_body_text(&e.payload);
                if !tool_result_looks_failed(&e.payload, &text) {
                    continue;
                }
                *by_session.entry(e.session_id.clone()).or_insert(0) += 1;
            }
            _ => {}
        }
    }

    let mut best: Option<(String, u32)> = None;
    for (sid, n) in by_session {
        if n < MIN_FAILING_RESULTS {
            continue;
        }
        best = Some(match best {
            None => (sid, n),
            Some((ref os, on)) if n > on => (sid, n),
            Some(o) => o,
        });
    }

    let Some((sid, n)) = best else {
        return vec![];
    };

    vec![Bet {
        id: format!("H10:{sid}"),
        heuristic_id: "H10".into(),
        title: "Repeated failing shell / test runs".into(),
        hypothesis: format!(
            "Session `{}` has {} failing terminal-style tool results — likely a tight fix loop or flaky checks.",
            sid, n
        ),
        expected_tokens_saved_per_week: (n as f64) * 400.0,
        effort_minutes: 45,
        evidence: vec![format!("Failing shell-like results in session: {}", n)],
        apply_step:
            "Stabilize the failing command (fixture, env, or smaller test target); add a pre-push script or CI signal so the agent stops thrashing."
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
    fn fires_on_three_failed_shell_results() {
        let mut agg = RetroAggregates::default();
        agg.unique_session_ids.insert("s1".into());
        let mut events = vec![];
        for i in 0..3u64 {
            let tid = format!("call_{i}");
            events.push((
                sess("s1"),
                Event {
                    session_id: "s1".into(),
                    seq: i * 2,
                    ts_ms: i * 2,
                    ts_exact: true,
                    kind: EventKind::ToolCall,
                    source: EventSource::Tail,
                    tool: Some("run_terminal_cmd".into()),
                    tool_call_id: Some(tid.clone()),
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
            ));
            events.push((
                sess("s1"),
                Event {
                    session_id: "s1".into(),
                    seq: i * 2 + 1,
                    ts_ms: i * 2 + 1,
                    ts_exact: true,
                    kind: EventKind::ToolResult,
                    source: EventSource::Tail,
                    tool: None,
                    tool_call_id: Some(tid),
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
                                        payload: json!({"is_error": true, "content": [{"type":"text","text":"exit code 1"}]}),
                },
            ));
        }
        let inputs = Inputs {
            window_start_ms: 0,
            window_end_ms: 999,
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
        assert_eq!(bets[0].heuristic_id, "H10");
    }
}
