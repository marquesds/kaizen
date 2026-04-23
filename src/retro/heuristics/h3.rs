//! H3 — Repeated churn on the same path in one session (edit loop proxy).

use crate::core::event::EventKind;
use crate::retro::types::{Bet, Inputs};
use serde_json::Value;
use std::collections::HashMap;

const MIN_TOUCHES: usize = 4;

fn path_touch_from_payload(payload: &Value) -> Option<String> {
    payload
        .get("input")
        .and_then(|i| i.get("path").and_then(|p| p.as_str()))
        .map(|s| s.to_string())
        .or_else(|| {
            payload
                .get("function")
                .and_then(|f| f.get("arguments"))
                .and_then(|a| a.as_str())
                .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
                .and_then(|v| {
                    v.get("path")
                        .and_then(|p| p.as_str().map(|s| s.to_string()))
                })
        })
}

pub fn run(inputs: &Inputs) -> Vec<Bet> {
    let mut by_session_path: HashMap<(String, String), u64> = HashMap::new();
    for (s, e) in &inputs.events {
        if e.kind != EventKind::ToolCall {
            continue;
        }
        if let Some(p) = path_touch_from_payload(&e.payload) {
            *by_session_path.entry((s.id.clone(), p)).or_default() += 1;
        }
    }

    let mut out = Vec::new();
    for ((sid, path), n) in by_session_path {
        if n < MIN_TOUCHES as u64 {
            continue;
        }
        let complexity = inputs
            .file_facts
            .get(&path)
            .map(|f| f.complexity_total)
            .unwrap_or(0);
        let churn = inputs
            .file_facts
            .get(&path)
            .map(|f| f.churn_30d)
            .unwrap_or(0);
        let id = format!("H3:{}:{}", sid, path);
        out.push(Bet {
            id,
            heuristic_id: "H3".into(),
            title: format!("Tighten guardrails for `{}`", path),
            hypothesis: format!(
                "Session `{}` issued {} tool calls targeting the same path — likely a failure loop.",
                sid, n
            ),
            expected_tokens_saved_per_week: (n as f64) * (800.0 + complexity as f64 * 20.0),
            effort_minutes: 45,
            evidence: vec![
                format!("Touches in single session: {}", n),
                format!("Complexity: {} · churn30: {}", complexity, churn),
            ],
            apply_step: format!(
                "Add regression test or invariant near `{}` so agents stop spinning.",
                path
            ),
            evidence_recency_ms: inputs.window_end_ms,
        });
    }
    out
}
