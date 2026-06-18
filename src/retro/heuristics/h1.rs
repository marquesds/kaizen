// SPDX-License-Identifier: AGPL-3.0-or-later
//! H1 — Dead skill or Cursor rule: on-disk file unused in lookback window, not edited recently.

use crate::retro::types::{Bet, Inputs};

const STALE_EDIT_MS: u64 = 60 * 86_400_000;
const MIN_EVENTS_FOR_CREDIBLE: usize = 24;

pub fn run(inputs: &Inputs) -> Vec<Bet> {
    if inputs.events.len() < MIN_EVENTS_FOR_CREDIBLE {
        return vec![];
    }
    let mut out = Vec::new();
    let now = inputs.window_end_ms;
    for sf in &inputs.skill_files_on_disk {
        if inputs.skills_used_recent_slugs.contains(&sf.slug) {
            continue;
        }
        // Skip skills touched on disk within the last 60 days.
        if sf.mtime_ms > now.saturating_sub(STALE_EDIT_MS) {
            continue;
        }
        let est_tokens_week = (sf.size_bytes as f64 / 4.0) * 10.0;
        let id = format!("H1:{}", sf.slug);
        out.push(Bet {
            id,
            heuristic_id: "H1".into(),
            title: format!("Remove or archive unused skill `{}`", sf.slug),
            hypothesis: format!(
                "Skill `.cursor/skills/{}/` has not been referenced in tracked sessions for the lookback window and was last modified more than 60 days ago.",
                sf.slug
            ),
            expected_tokens_saved_per_week: est_tokens_week,
            effort_minutes: 5,
            evidence: vec![format!(
                "On-disk size ~{} bytes; not in recent `skills_used` index.",
                sf.size_bytes
            )],
            apply_step: format!(
                "Run `kaizen guidance propose --artifact skill:{}` and review the suggested change.",
                sf.slug
            ),
            evidence_recency_ms: sf.mtime_ms,
        confidence: None,
        category: None,
        });
    }
    for rf in &inputs.rule_files_on_disk {
        if inputs.rules_used_recent_slugs.contains(&rf.slug) {
            continue;
        }
        if rf.mtime_ms > now.saturating_sub(STALE_EDIT_MS) {
            continue;
        }
        let est_tokens_week = (rf.size_bytes as f64 / 4.0) * 10.0;
        let id = format!("H1r:{}", rf.slug);
        out.push(Bet {
            id,
            heuristic_id: "H1".into(),
            title: format!("Remove or archive unused rule `{}`", rf.slug),
            hypothesis: format!(
                "Rule `.cursor/rules/{}.mdc` has not been referenced in tracked sessions for the lookback window and was last modified more than 60 days ago.",
                rf.slug
            ),
            expected_tokens_saved_per_week: est_tokens_week,
            effort_minutes: 5,
            evidence: vec![format!(
                "On-disk size ~{} bytes; not in recent `rules_used` index.",
                rf.size_bytes
            )],
            apply_step: format!(
                "Run `kaizen guidance propose --artifact rule:{}` and review the suggested change.",
                rf.slug
            ),
            evidence_recency_ms: rf.mtime_ms,
        confidence: None,
        category: None,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::event::{Event, EventKind, EventSource, SessionRecord, SessionStatus};
    use crate::retro::types::{RetroAggregates, SkillFileOnDisk};
    use serde_json::json;
    use std::collections::{HashMap, HashSet};

    fn base_inputs() -> Inputs {
        let mut agg = RetroAggregates::default();
        agg.unique_session_ids.insert("s1".into());
        Inputs {
            window_start_ms: 0,
            window_end_ms: 100_000_000,
            events: (0..30)
                .map(|i| {
                    (
                        SessionRecord {
                            id: "s1".into(),
                            agent: "cursor".into(),
                            model: None,
                            workspace: "/w".into(),
                            started_at_ms: 0,
                            ended_at_ms: None,
                            status: SessionStatus::Done,
                            trace_path: "".into(),
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
                        },
                        Event {
                            session_id: "s1".into(),
                            seq: i,
                            ts_ms: i * 1000,
                            ts_exact: false,
                            kind: EventKind::ToolCall,
                            source: EventSource::Tail,
                            tool: Some("read_file".into()),
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
                            payload: json!({}),
                        },
                    )
                })
                .collect(),
            files_touched: vec![],
            skills_used: vec![],
            tool_spans: vec![],
            skills_used_recent_slugs: HashSet::new(),
            usage_lookback_ms: 30 * 86_400_000,
            skill_files_on_disk: vec![SkillFileOnDisk {
                slug: "dead".into(),
                size_bytes: 400,
                mtime_ms: 0,
            }],
            rule_files_on_disk: vec![],
            rules_used_recent_slugs: HashSet::new(),
            file_facts: HashMap::new(),
            eval_scores: vec![],
            aggregates: agg,
            prompt_fingerprints: vec![],
            feedback: vec![],
            session_outcomes: vec![],
            session_sample_aggs: vec![],
        }
    }

    #[test]
    fn finds_dead_skill_when_stale() {
        let bets = run(&base_inputs());
        assert_eq!(bets.len(), 1);
        assert_eq!(bets[0].heuristic_id, "H1");
    }

    #[test]
    fn skips_recently_used_slug() {
        let mut i = base_inputs();
        i.skills_used_recent_slugs.insert("dead".into());
        assert!(run(&i).is_empty());
    }
}
