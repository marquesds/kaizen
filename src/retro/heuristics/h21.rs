// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::retro::types::{Bet, Inputs};
use std::collections::HashMap;

const SESSION_RETRY_TRIGGER: u32 = 15;
const HIGH_RETRY_PER_EVENT: u16 = 3;
const SESSIONS_WITH_HIGH_RETRY: usize = 5;

pub fn run(inputs: &Inputs) -> Vec<Bet> {
    let (max_session_retries, session_id, sessions_high) = scan_retries(inputs);
    let triggered =
        max_session_retries >= SESSION_RETRY_TRIGGER || sessions_high >= SESSIONS_WITH_HIGH_RETRY;
    if !triggered {
        return vec![];
    }
    let evidence = if max_session_retries >= SESSION_RETRY_TRIGGER {
        format!(
            "session {} had {max_session_retries} retries; {sessions_high} sessions with ≥{HIGH_RETRY_PER_EVENT} retries/call",
            session_id
        )
    } else {
        format!("{sessions_high} sessions with ≥{HIGH_RETRY_PER_EVENT} retries per call")
    };
    vec![Bet {
        id: format!("H21:retries:{max_session_retries}:{sessions_high}"),
        heuristic_id: "H21".into(),
        title: format!("Rate-limit cascades detected ({sessions_high} sessions affected)"),
        hypothesis:
            "Repeated retries inflate latency and cost. Model routing or batch-window is too narrow."
                .into(),
        expected_tokens_saved_per_week: sessions_high as f64 * 1_500.0,
        effort_minutes: 45,
        evidence: vec![evidence],
        apply_step:
            "Route to lower-tier model during peak or widen the request batch window.".into(),
        evidence_recency_ms: inputs.window_end_ms,
    confidence: None,
    category: None,
    }]
}

fn scan_retries(inputs: &Inputs) -> (u32, String, usize) {
    let mut session_totals: HashMap<&str, u32> = HashMap::new();
    for (session, event) in &inputs.events {
        let Some(rc) = event.retry_count else {
            continue;
        };
        *session_totals.entry(session.id.as_str()).or_insert(0) += rc as u32;
    }
    let max_entry = session_totals
        .iter()
        .max_by_key(|(_, v)| *v)
        .map(|(id, &v)| (v, id.to_string()))
        .unwrap_or((0, String::new()));
    let sessions_high = session_totals
        .values()
        .filter(|&&v| v >= HIGH_RETRY_PER_EVENT as u32)
        .count();
    (max_entry.0, max_entry.1, sessions_high)
}
