// SPDX-License-Identifier: AGPL-3.0-or-later
//! Repeating length-2 / length-3 tool patterns (non-overlapping maximal blocks).

use crate::core::event::EventKind;
use crate::retro::types::{Bet, Inputs};
use std::collections::{HashMap, HashSet};

const MIN_SUBSEQ_REPEATS_LEN2: usize = 3;
const MIN_SUBSEQ_REPEATS_LEN3: usize = 2;
const TOKENS_PER_CYCLE: f64 = 200.0;
const MULTI_SESSION_MULT: f64 = 1.25;
const EFFORT_MINUTES: u32 = 25;

pub fn run(inputs: &Inputs) -> Vec<Bet> {
    build_subseq_bets(
        merge_pattern_counts(scan_sessions(inputs)),
        inputs.window_end_ms,
    )
}

fn scan_sessions(inputs: &Inputs) -> HashMap<Vec<String>, Vec<(String, usize)>> {
    let mut acc: HashMap<Vec<String>, Vec<(String, usize)>> = HashMap::new();
    for (sid, tools) in group_session_tools(inputs) {
        let sl: Vec<&str> = tools.iter().map(|x| x.as_str()).collect();
        for k in [2usize, 3] {
            for (pat, reps) in find_repeating_subseqs(&sl, k) {
                acc.entry(pat).or_default().push((sid.clone(), reps));
            }
        }
    }
    acc
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

fn merge_pattern_counts(
    raw: HashMap<Vec<String>, Vec<(String, usize)>>,
) -> HashMap<Vec<String>, (usize, HashSet<String>)> {
    let mut out: HashMap<Vec<String>, (usize, HashSet<String>)> = HashMap::new();
    for (pat, rows) in raw {
        let mut per_sess: HashMap<String, usize> = HashMap::new();
        for (sid, r) in rows {
            *per_sess.entry(sid).or_insert(0) += r;
        }
        let mut sessions = HashSet::new();
        let mut total = 0usize;
        for (sid, r) in per_sess {
            total += r;
            sessions.insert(sid);
        }
        out.insert(pat, (total, sessions));
    }
    out
}

fn build_subseq_bets(
    patterns: HashMap<Vec<String>, (usize, HashSet<String>)>,
    window_end_ms: u64,
) -> Vec<Bet> {
    let mut out = Vec::new();
    for (pat, (total_repeats, sessions)) in patterns {
        let mult = if sessions.len() >= 2 {
            MULTI_SESSION_MULT
        } else {
            1.0
        };
        out.push(mk_subseq_bet(
            &pat,
            total_repeats,
            &sessions,
            mult,
            window_end_ms,
        ));
    }
    out
}

fn mk_subseq_bet(
    pat: &[String],
    total_repeats: usize,
    sessions: &HashSet<String>,
    mult: f64,
    window_end_ms: u64,
) -> Bet {
    let key = pat.join("+");
    let mut sess_list: Vec<String> = sessions.iter().cloned().collect();
    sess_list.sort();
    let sess_note = format!("sessions={}", sess_list.len());
    Bet {
        id: format!("H33:subseq:{key}"),
        heuristic_id: "H33".into(),
        title: "Repeating tool micro-workflow".into(),
        hypothesis: format!(
            "Pattern `{key}` repeats {total_repeats}× — a script or skill could collapse the loop."
        ),
        expected_tokens_saved_per_week: TOKENS_PER_CYCLE * (total_repeats as f64) * mult,
        effort_minutes: EFFORT_MINUTES,
        evidence: vec![sess_note, format!("pattern={key}")],
        apply_step: "Capture as a small script, Justfile target, or Cursor skill with parameters."
            .into(),
        evidence_recency_ms: window_end_ms,
        confidence: None,
        category: None,
    }
}

/// Non-overlapping maximal repeats of a length-`k` block (`k` is 2 or 3).
pub(crate) fn find_repeating_subseqs(tools: &[&str], k: usize) -> Vec<(Vec<String>, usize)> {
    let min_rep = match k {
        2 => MIN_SUBSEQ_REPEATS_LEN2,
        3 => MIN_SUBSEQ_REPEATS_LEN3,
        _ => return vec![],
    };
    let mut out = Vec::new();
    let mut i = 0;
    while i + 2 * k <= tools.len() {
        if tools[i..i + k] != tools[i + k..i + 2 * k] {
            i += 1;
            continue;
        }
        let mut repeats = 2usize;
        let mut pos = i + 2 * k;
        while pos + k <= tools.len() && tools[pos..pos + k] == tools[i..i + k] {
            repeats += 1;
            pos += k;
        }
        if repeats >= min_rep {
            let pat: Vec<String> = tools[i..i + k].iter().map(|s| (*s).to_string()).collect();
            out.push((pat, repeats));
        }
        i += repeats * k;
    }
    out
}
