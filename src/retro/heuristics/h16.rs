// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::retro::types::{Bet, Inputs};
use std::collections::HashMap;

const MIN_SESSIONS_PER_FP: usize = 5;
const COST_DIFF_THRESHOLD: f64 = 0.20;
const ERROR_DIFF_THRESHOLD: f64 = 0.15;
const TOKENS_SAVED_PER_SESSION: f64 = 500.0;

pub fn run(inputs: &Inputs) -> Vec<Bet> {
    let fps = &inputs.prompt_fingerprints;
    if fps.len() < MIN_SESSIONS_PER_FP * 2 {
        return vec![];
    }
    let groups = group_by_fingerprint(fps);
    if groups.len() < 2 {
        return vec![];
    }
    let cost_map = cost_by_session(inputs);
    let error_map = error_rate_by_session(inputs);
    compare_fingerprints(&groups, &cost_map, &error_map, inputs.window_end_ms)
}

fn group_by_fingerprint(fps: &[(String, String)]) -> HashMap<String, Vec<String>> {
    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    for (sid, fp) in fps {
        map.entry(fp.clone()).or_default().push(sid.clone());
    }
    map.retain(|_, v| v.len() >= MIN_SESSIONS_PER_FP);
    map
}

fn cost_by_session(inputs: &Inputs) -> HashMap<String, i64> {
    let mut map: HashMap<String, i64> = HashMap::new();
    for (s, e) in &inputs.events {
        if let Some(c) = e.cost_usd_e6 {
            *map.entry(s.id.clone()).or_default() += c;
        }
    }
    map
}

fn error_rate_by_session(inputs: &Inputs) -> HashMap<String, f64> {
    let mut total: HashMap<String, u64> = HashMap::new();
    let mut errors: HashMap<String, u64> = HashMap::new();
    for (s, e) in &inputs.events {
        *total.entry(s.id.clone()).or_default() += 1;
        if matches!(e.kind, crate::core::event::EventKind::Error) {
            *errors.entry(s.id.clone()).or_default() += 1;
        }
    }
    total
        .into_iter()
        .map(|(id, n)| {
            let err = *errors.get(&id).unwrap_or(&0) as f64;
            (id, if n > 0 { err / n as f64 } else { 0.0 })
        })
        .collect()
}

fn compare_fingerprints(
    groups: &HashMap<String, Vec<String>>,
    cost_map: &HashMap<String, i64>,
    error_map: &HashMap<String, f64>,
    recency_ms: u64,
) -> Vec<Bet> {
    let fps: Vec<&String> = groups.keys().collect();
    let mut bets = Vec::new();
    for i in 0..fps.len() {
        for j in (i + 1)..fps.len() {
            let a = fps[i];
            let b = fps[j];
            let sessions_a = &groups[a];
            let sessions_b = &groups[b];
            if let Some(bet) = maybe_bet(
                a, sessions_a, b, sessions_b, cost_map, error_map, recency_ms,
            ) {
                bets.push(bet);
            }
        }
    }
    bets
}

fn mean_cost(sessions: &[String], cost_map: &HashMap<String, i64>) -> f64 {
    if sessions.is_empty() {
        return 0.0;
    }
    let total: i64 = sessions.iter().filter_map(|id| cost_map.get(id)).sum();
    total as f64 / sessions.len() as f64
}

fn mean_error_rate(sessions: &[String], error_map: &HashMap<String, f64>) -> f64 {
    if sessions.is_empty() {
        return 0.0;
    }
    let sum: f64 = sessions.iter().filter_map(|id| error_map.get(id)).sum();
    sum / sessions.len() as f64
}

struct FpStats<'a> {
    fp: &'a str,
    sessions: &'a [String],
    mean_cost: f64,
    mean_err: f64,
}

fn maybe_bet(
    a: &str,
    sessions_a: &[String],
    b: &str,
    sessions_b: &[String],
    cost_map: &HashMap<String, i64>,
    error_map: &HashMap<String, f64>,
    recency_ms: u64,
) -> Option<Bet> {
    let sa = FpStats {
        fp: a,
        sessions: sessions_a,
        mean_cost: mean_cost(sessions_a, cost_map),
        mean_err: mean_error_rate(sessions_a, error_map),
    };
    let sb = FpStats {
        fp: b,
        sessions: sessions_b,
        mean_cost: mean_cost(sessions_b, cost_map),
        mean_err: mean_error_rate(sessions_b, error_map),
    };
    let (worse, better, metric, delta) = select_worse(&sa, &sb)?;
    let short_worse = &worse.fp[..8.min(worse.fp.len())];
    let short_better = &better.fp[..8.min(better.fp.len())];
    Some(Bet {
        id: format!("H16:{short_worse}|{short_better}"),
        heuristic_id: "H16".into(),
        title: format!(
            "Prompt {short_worse} underperforms {short_better} — {metric} diff {delta:.0}%"
        ),
        hypothesis: format!(
            "Prompt {short_worse} has worse {metric} ({delta:.1}%) vs {short_better} over {} sessions.",
            worse.sessions.len()
        ),
        expected_tokens_saved_per_week: worse.sessions.len() as f64 * TOKENS_SAVED_PER_SESSION,
        effort_minutes: 20,
        evidence: worse.sessions.iter().take(5).cloned().collect(),
        apply_step: format!(
            "Run `kaizen prompt diff {short_worse} {short_better}` to inspect changes."
        ),
        evidence_recency_ms: recency_ms,
    })
}

fn select_worse<'a>(
    a: &'a FpStats<'a>,
    b: &'a FpStats<'a>,
) -> Option<(&'a FpStats<'a>, &'a FpStats<'a>, &'static str, f64)> {
    let cost_diff = rel_diff(a.mean_cost, b.mean_cost);
    let err_diff = rel_diff(a.mean_err, b.mean_err);
    if cost_diff.abs() >= COST_DIFF_THRESHOLD {
        return if cost_diff > 0.0 {
            Some((a, b, "cost", cost_diff * 100.0))
        } else {
            Some((b, a, "cost", -cost_diff * 100.0))
        };
    }
    if err_diff.abs() >= ERROR_DIFF_THRESHOLD {
        return if err_diff > 0.0 {
            Some((a, b, "error_rate", err_diff * 100.0))
        } else {
            Some((b, a, "error_rate", -err_diff * 100.0))
        };
    }
    None
}

fn rel_diff(a: f64, b: f64) -> f64 {
    if b == 0.0 {
        return 0.0;
    }
    (a - b) / b
}
