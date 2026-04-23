// SPDX-License-Identifier: AGPL-3.0-or-later
//! H8 — Doc drift: reads under `docs/` then edits elsewhere in the same session.

use crate::core::event::EventKind;
use crate::retro::types::{Bet, Inputs};
use serde_json::Value;

fn path_from_tool_payload(payload: &Value) -> Option<String> {
    payload
        .get("input")
        .and_then(|i| i.get("path").and_then(|p| p.as_str()))
        .map(|s| s.to_string())
}

pub fn run(inputs: &Inputs) -> Vec<Bet> {
    let mut by_session: std::collections::HashMap<String, Vec<&crate::core::event::Event>> =
        std::collections::HashMap::new();
    for (_, e) in &inputs.events {
        by_session.entry(e.session_id.clone()).or_default().push(e);
    }
    let mut out = Vec::new();
    for (sid, evs) in by_session {
        let mut saw_doc_read = false;
        let mut drift_hits = 0u64;
        let mut touched_paths = vec![];
        for e in evs {
            if e.kind != EventKind::ToolCall {
                continue;
            }
            let Some(p) = path_from_tool_payload(&e.payload) else {
                continue;
            };
            let pl = p.to_lowercase();
            if pl.starts_with("docs/") || pl.contains("/docs/") {
                saw_doc_read = true;
                continue;
            }
            if saw_doc_read && (pl.ends_with(".rs") || pl.ends_with(".ts") || pl.ends_with(".md")) {
                drift_hits += 1;
                touched_paths.push(p);
            }
        }
        if drift_hits >= 3 {
            let complexity = touched_paths
                .iter()
                .filter_map(|path| inputs.file_facts.get(path))
                .map(|fact| fact.complexity_total)
                .sum::<u32>();
            let id = format!("H8:{sid}");
            out.push(Bet {
                id,
                heuristic_id: "H8".into(),
                title: "Docs may be stale vs implementation".into(),
                hypothesis: format!(
                    "Session `{}` read docs then edited {} implementation files — agents may be reconciling drift.",
                    sid, drift_hits
                ),
                expected_tokens_saved_per_week: (drift_hits as f64) * (600.0 + complexity as f64 * 10.0),
                effort_minutes: 40,
                evidence: vec![
                    format!("Doc-read → code-edit pattern count: {}", drift_hits),
                    format!("Touched impl complexity sum: {}", complexity),
                ],
                apply_step: "Refresh the touched docs to match code, or add a single source of truth link.".into(),
                evidence_recency_ms: inputs.window_end_ms,
            });
        }
    }
    out
}
