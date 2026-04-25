// SPDX-License-Identifier: AGPL-3.0-or-later
//! H2 — Hot file cluster: pairs of files co-edited often across sessions.

use crate::retro::types::{Bet, Inputs};
use std::collections::{HashMap, HashSet};

const MIN_PAIR_SESSIONS: u64 = 3;

fn top_path_component(path: &str) -> &str {
    path.trim_start_matches('/').split('/').next().unwrap_or("")
}

pub fn run(inputs: &Inputs) -> Vec<Bet> {
    let mut by_session: HashMap<String, HashSet<String>> = HashMap::new();
    for (sid, path) in &inputs.files_touched {
        by_session
            .entry(sid.clone())
            .or_default()
            .insert(path.clone());
    }
    let mut pair_counts: HashMap<(String, String), u64> = HashMap::new();
    for paths in by_session.values() {
        let list: Vec<String> = paths.iter().cloned().collect();
        for i in 0..list.len() {
            for j in (i + 1)..list.len() {
                let mut a = list[i].clone();
                let mut b = list[j].clone();
                if a > b {
                    std::mem::swap(&mut a, &mut b);
                }
                *pair_counts.entry((a, b)).or_default() += 1;
            }
        }
    }

    let mut out = Vec::new();
    for ((a, b), n) in pair_counts {
        if n < MIN_PAIR_SESSIONS {
            continue;
        }
        let ca = top_path_component(&a);
        let cb = top_path_component(&b);
        if ca == cb || ca.is_empty() || cb.is_empty() {
            continue;
        }
        let id = format!("H2:{}|{}", a, b);
        let complexity = inputs
            .file_facts
            .get(&a)
            .map(|f| f.complexity_total)
            .unwrap_or(0)
            + inputs
                .file_facts
                .get(&b)
                .map(|f| f.complexity_total)
                .unwrap_or(0);
        let est = (n as f64) * (500.0 + complexity as f64 * 15.0);
        out.push(Bet {
            id,
            heuristic_id: "H2".into(),
            title: format!("Hidden coupling: `{}` ↔ `{}`", a, b),
            hypothesis: format!(
                "These files are edited together in {} distinct sessions — likely shared context cost for agents.",
                n
            ),
            expected_tokens_saved_per_week: est,
            effort_minutes: 120,
            evidence: vec![
                format!("Co-edit count: {} (cross-module).", n),
                format!("Combined complexity: {}", complexity),
            ],
            apply_step: format!(
                "Extract shared logic or add a facade so agents touch one module instead of `{}` + `{}`.",
                a, b
            ),
            evidence_recency_ms: inputs.window_end_ms,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retro::types::RetroAggregates;
    use std::collections::HashSet;

    #[test]
    fn pair_cross_module_triggers() {
        let mut agg = RetroAggregates::default();
        agg.unique_session_ids.insert("s1".into());
        agg.unique_session_ids.insert("s2".into());
        agg.unique_session_ids.insert("s3".into());
        let ft: Vec<_> = (1..=3)
            .flat_map(|i| {
                let sid = format!("s{i}");
                vec![
                    (sid.clone(), "src/a.rs".into()),
                    (sid, "crates/b/src/lib.rs".into()),
                ]
            })
            .collect();
        let inputs = Inputs {
            window_start_ms: 0,
            window_end_ms: 1,
            events: vec![],
            files_touched: ft,
            skills_used: vec![],
            tool_spans: vec![],
            skills_used_recent_slugs: HashSet::new(),
            usage_lookback_ms: 0,
            skill_files_on_disk: vec![],
            rule_files_on_disk: vec![],
            rules_used_recent_slugs: HashSet::new(),
            file_facts: HashMap::new(),
            eval_scores: vec![],
            aggregates: agg,
        };
        let bets = run(&inputs);
        assert_eq!(bets.len(), 1);
        assert_eq!(bets[0].heuristic_id, "H2");
    }
}
