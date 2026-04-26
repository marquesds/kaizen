// SPDX-License-Identifier: AGPL-3.0-or-later
//! H12 — Large file reads: read-like tools hitting paths with high LOC/bytes in `file_facts`.
//!
//! Degrades gracefully when `file_facts` is empty (no bet).

use crate::core::event::EventKind;
use crate::retro::types::{Bet, Inputs};
use crate::store::event_index::paths_from_event_payload;
use std::collections::HashMap;

const MIN_LOC: u32 = 500;
const MIN_BYTES: u64 = 80_000;
/// Need at least this many large-file read tool calls targeting the winning path.
const MIN_READS_ON_PATH: u64 = 2;

fn read_like_tool(name: &str) -> bool {
    let n = name.to_lowercase();
    n.contains("read_file")
        || n == "read"
        || n.contains("file_read")
        || n.contains("view_file")
        || n == "open_file"
        || n.contains("/read")
}

fn is_large_fact(path: &str, inputs: &Inputs) -> bool {
    let Some(f) = inputs.file_facts.get(path) else {
        return false;
    };
    f.loc >= MIN_LOC || f.bytes >= MIN_BYTES
}

pub fn run(inputs: &Inputs) -> Vec<Bet> {
    if inputs.file_facts.is_empty() {
        return vec![];
    }
    let mut counts: HashMap<String, u64> = HashMap::new();
    for (_, e) in &inputs.events {
        if e.kind != EventKind::ToolCall {
            continue;
        }
        let Some(tool) = e.tool.as_deref() else {
            continue;
        };
        if !read_like_tool(tool) {
            continue;
        }
        for p in paths_from_event_payload(&e.payload) {
            if is_large_fact(&p, inputs) {
                *counts.entry(p).or_default() += 1;
            }
        }
    }
    let Some((path, n)) = counts.into_iter().max_by_key(|(_, c)| *c) else {
        return vec![];
    };
    if n < MIN_READS_ON_PATH {
        return vec![];
    }
    let Some(fact) = inputs.file_facts.get(&path) else {
        return vec![];
    };
    vec![Bet {
        id: format!("H12:{}", path.replace('/', "|")),
        heuristic_id: "H12".into(),
        title: format!("Repeated reads of large file `{}`", path),
        hypothesis: format!(
            "Agents called read-like tools {}× on `{}` (LOC {}, {} bytes) — context cost adds up.",
            n, path, fact.loc, fact.bytes
        ),
        expected_tokens_saved_per_week: (n as f64) * (fact.loc as f64) * 2.0,
        effort_minutes: 40,
        evidence: vec![
            format!("Read-like calls on path: {}", n),
            format!("LOC: {} · bytes: {}", fact.loc, fact.bytes),
        ],
        apply_step: format!(
            "Add a read-hygiene rule; split `{}` or point agents at a smaller module/summary first.",
            path
        ),
        evidence_recency_ms: inputs.window_end_ms,
    }]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::event::{Event, EventSource, SessionRecord, SessionStatus};
    use crate::metrics::types::FileFact;
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
    fn fires_on_large_file_reads() {
        let mut agg = RetroAggregates::default();
        agg.unique_session_ids.insert("s1".into());
        let mut file_facts = HashMap::new();
        file_facts.insert(
            "src/huge.rs".into(),
            FileFact {
                snapshot_id: "snap".into(),
                path: "src/huge.rs".into(),
                language: "rust".into(),
                bytes: 90_000,
                loc: 2000,
                sloc: 1800,
                complexity_total: 10,
                max_fn_complexity: 3,
                symbol_count: 5,
                import_count: 2,
                fan_in: 1,
                fan_out: 2,
                churn_30d: 0,
                churn_90d: 0,
                authors_90d: 1,
                last_changed_ms: None,
            },
        );
        let events: Vec<_> = (0..2u64)
            .map(|i| {
                (
                    sess("s1"),
                    Event {
                        session_id: "s1".into(),
                        seq: i,
                        ts_ms: i,
                        ts_exact: true,
                        kind: EventKind::ToolCall,
                        source: EventSource::Tail,
                        tool: Some("read_file".into()),
                        tool_call_id: Some(format!("c{i}")),
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
                        payload: json!({"input": {"path": "src/huge.rs"}}),
                    },
                )
            })
            .collect();
        let inputs = Inputs {
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
            file_facts,
            eval_scores: vec![],
            aggregates: agg,
            prompt_fingerprints: vec![],
            feedback: vec![],
            session_outcomes: vec![],
            session_sample_aggs: vec![],
        };
        let bets = run(&inputs);
        assert_eq!(bets.len(), 1);
        assert_eq!(bets[0].heuristic_id, "H12");
    }
}
