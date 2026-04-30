// SPDX-License-Identifier: AGPL-3.0-or-later
//! H14 — Instruction surface bloat: many skills and/or rules on disk with large combined payload.

use crate::retro::types::{Bet, Inputs};

/// Fire when combined count of skill dirs + rule files is at least this.
const MIN_COMBINED_ITEMS: usize = 22;
/// Or when total bytes of SKILL.md + `.mdc` bodies exceeds this (with at least MIN_ITEMS_WITH_BYTES items).
const MIN_TOTAL_BYTES: u64 = 140_000;
const MIN_ITEMS_WITH_BYTES: usize = 10;

pub fn run(inputs: &Inputs) -> Vec<Bet> {
    let n_skills = inputs.skill_files_on_disk.len();
    let n_rules = inputs.rule_files_on_disk.len();
    let n = n_skills + n_rules;
    let total_bytes: u64 = inputs
        .skill_files_on_disk
        .iter()
        .map(|s| s.size_bytes)
        .chain(inputs.rule_files_on_disk.iter().map(|r| r.size_bytes))
        .sum();
    let bytes_ok = total_bytes >= MIN_TOTAL_BYTES && n >= MIN_ITEMS_WITH_BYTES;
    if n < MIN_COMBINED_ITEMS && !bytes_ok {
        return vec![];
    }
    vec![Bet {
        id: "H14:instruction-surface".into(),
        heuristic_id: "H14".into(),
        title: "Large always-on instruction surface".into(),
        hypothesis: format!(
            "{} skill dirs + {} rule files (~{} KiB raw) — models pay context for descriptions and rule text even when irrelevant.",
            n_skills,
            n_rules,
            total_bytes / 1024
        ),
        expected_tokens_saved_per_week: (total_bytes as f64) / 8.0,
        effort_minutes: 50,
        evidence: vec![
            format!("Skills on disk: {}", n_skills),
            format!("Rules on disk: {}", n_rules),
            format!("Approx combined bytes: {}", total_bytes),
        ],
        apply_step:
            "Merge overlapping rules, archive stale skills, move rarely-used guidance to docs with explicit `@` references."
                .into(),
        evidence_recency_ms: inputs.window_end_ms,
    confidence: None,
    category: None,
    }]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retro::types::{RetroAggregates, SkillFileOnDisk};
    use std::collections::HashSet;

    #[test]
    fn fires_on_many_items() {
        let skills: Vec<_> = (0..25)
            .map(|i| SkillFileOnDisk {
                slug: format!("s{i}"),
                size_bytes: 100,
                mtime_ms: 0,
            })
            .collect();
        let inputs = Inputs {
            window_start_ms: 0,
            window_end_ms: 1,
            events: vec![],
            files_touched: vec![],
            skills_used: vec![],
            tool_spans: vec![],
            skills_used_recent_slugs: HashSet::new(),
            usage_lookback_ms: 0,
            skill_files_on_disk: skills,
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
        assert_eq!(bets.len(), 1);
        assert_eq!(bets[0].heuristic_id, "H14");
    }
}
