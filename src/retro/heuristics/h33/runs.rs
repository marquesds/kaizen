// SPDX-License-Identifier: AGPL-3.0-or-later
//! Same-tool maximal consecutive runs per session.

use crate::core::event::EventKind;
use crate::retro::types::{Bet, Inputs};
use std::collections::{HashMap, HashSet};

const MIN_RUN_LEN: usize = 5;
const MIN_SESSIONS_WITH_RUN: usize = 1;
const DOUBLE_RUN_LEN: usize = 10;
const TOKENS_PER_EXTRA_CALL: f64 = 150.0;
const EFFORT_MINUTES: u32 = 20;

pub fn run(inputs: &Inputs) -> Vec<Bet> {
    build_run_bets(
        &collect_by_tool(group_session_tools(inputs)),
        inputs.window_end_ms,
    )
}

fn group_session_tools(inputs: &Inputs) -> HashMap<String, Vec<String>> {
    let mut m: HashMap<String, Vec<String>> = HashMap::new();
    for (s, e) in &inputs.events {
        if e.kind != EventKind::ToolCall {
            continue;
        }
        let Some(t) = e.tool.as_ref() else {
            continue;
        };
        m.entry(s.id.clone()).or_default().push(t.clone());
    }
    m
}

fn collect_by_tool(
    sessions: HashMap<String, Vec<String>>,
) -> HashMap<String, Vec<(String, usize)>> {
    let mut by_tool: HashMap<String, Vec<(String, usize)>> = HashMap::new();
    for (sid, tools) in sessions {
        for (tool, len) in runs_from_tool_seq(&tools.iter().map(|s| s.as_str()).collect::<Vec<_>>())
        {
            if len < MIN_RUN_LEN {
                continue;
            }
            by_tool.entry(tool).or_default().push((sid.clone(), len));
        }
    }
    by_tool
}

/// Maximal same-tool segments: `(tool_name, segment_length)` in order.
pub(crate) fn runs_from_tool_seq(tools: &[&str]) -> Vec<(String, usize)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < tools.len() {
        let t = tools[i];
        let mut j = i + 1;
        while j < tools.len() && tools[j] == t {
            j += 1;
        }
        out.push((t.to_string(), j - i));
        i = j;
    }
    out
}

fn run_gate(n_sessions: usize, max_len: usize) -> bool {
    n_sessions >= MIN_SESSIONS_WITH_RUN || max_len >= DOUBLE_RUN_LEN
}

fn build_run_bets(by_tool: &HashMap<String, Vec<(String, usize)>>, window_end_ms: u64) -> Vec<Bet> {
    let mut out = Vec::new();
    for (tool, runs) in by_tool {
        let n_sess = runs.iter().map(|(s, _)| s).collect::<HashSet<_>>().len();
        let max_len = runs.iter().map(|(_, l)| *l).max().unwrap_or(0);
        if !run_gate(n_sess, max_len) {
            continue;
        }
        let extra: f64 = runs
            .iter()
            .filter(|(_, len)| *len >= MIN_RUN_LEN)
            .map(|(_, len)| (len - 1) as f64)
            .sum();
        out.push(mk_run_bet(tool, runs, max_len, extra, window_end_ms));
    }
    out
}

fn mk_run_bet(
    tool: &str,
    runs: &[(String, usize)],
    max_len: usize,
    extra_sum: f64,
    window_end_ms: u64,
) -> Bet {
    let mut ranked: Vec<(String, usize)> = runs.to_vec();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let evidence: Vec<String> = ranked
        .into_iter()
        .take(3)
        .map(|(sid, len)| format!("session={sid} len={len}"))
        .collect();
    Bet {
        id: format!("H33:run:{tool}"),
        heuristic_id: "H33".into(),
        title: "Repeated same tool in a row".into(),
        hypothesis: format!(
            "Tool `{tool}` appears in runs of up to {max_len} consecutive calls — batch or script may cut round-trips."
        ),
        expected_tokens_saved_per_week: TOKENS_PER_EXTRA_CALL * extra_sum,
        effort_minutes: EFFORT_MINUTES,
        evidence,
        apply_step:
            "Add a batch helper, glob, or short shell loop so one invocation replaces the streak."
                .into(),
        evidence_recency_ms: window_end_ms,
        confidence: None,
        category: None,
    }
}
