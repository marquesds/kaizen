// SPDX-License-Identifier: AGPL-3.0-or-later
//! Compute a scalar `Metric` value per session from its event stream.

use crate::core::event::{Event, EventKind, SessionRecord};
use crate::experiment::types::Metric;
use std::collections::HashSet;

/// Per-session metric value. `None` when the session has no basis for it
/// (e.g. no ended_at for DurationMinutes).
pub fn value_for(metric: Metric, session: &SessionRecord, events: &[Event]) -> Option<f64> {
    match metric {
        Metric::TokensPerSession => Some(tokens(events)),
        Metric::CostPerSession => Some(cost_usd(events)),
        Metric::SuccessRate => Some(success_rate(events)),
        Metric::ToolLoops => Some(tool_loops(events)),
        Metric::DurationMinutes => duration_minutes(session),
        Metric::FilesPerSession => Some(files_touched(events)),
    }
}

fn tokens(events: &[Event]) -> f64 {
    events
        .iter()
        .map(|e| {
            e.tokens_in.unwrap_or(0) as u64
                + e.tokens_out.unwrap_or(0) as u64
                + e.reasoning_tokens.unwrap_or(0) as u64
        })
        .sum::<u64>() as f64
}

fn cost_usd(events: &[Event]) -> f64 {
    events
        .iter()
        .map(|e| e.cost_usd_e6.unwrap_or(0))
        .sum::<i64>() as f64
        / 1_000_000.0
}

fn success_rate(events: &[Event]) -> f64 {
    if events.iter().any(|e| matches!(e.kind, EventKind::Error)) {
        0.0
    } else {
        1.0
    }
}

/// Number of consecutive repeats of the same `tool` — rough
/// edit→test_fail→edit proxy. Exact count without requires a span-aware
/// rewrite; event-stream approximation suffices for v0 binding.
fn tool_loops(events: &[Event]) -> f64 {
    let mut loops = 0_u64;
    let mut last: Option<&str> = None;
    for e in events {
        if e.kind != EventKind::ToolCall {
            continue;
        }
        let Some(tool) = e.tool.as_deref() else {
            continue;
        };
        if last == Some(tool) {
            loops += 1;
        }
        last = Some(tool);
    }
    loops as f64
}

fn duration_minutes(session: &SessionRecord) -> Option<f64> {
    let end = session.ended_at_ms?;
    Some((end.saturating_sub(session.started_at_ms) as f64) / 60_000.0)
}

fn files_touched(events: &[Event]) -> f64 {
    let mut files: HashSet<String> = HashSet::new();
    for e in events {
        let Some(path) = e
            .payload
            .get("input")
            .and_then(|o| o.get("path"))
            .and_then(|v| v.as_str())
            .or_else(|| e.payload.get("path").and_then(|v| v.as_str()))
        else {
            continue;
        };
        files.insert(path.to_string());
    }
    files.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::event::{EventSource, SessionStatus};
    use serde_json::json;

    fn ev(kind: EventKind, tool: Option<&str>) -> Event {
        Event {
            session_id: "s".into(),
            seq: 0,
            ts_ms: 0,
            ts_exact: false,
            kind,
            source: EventSource::Tail,
            tool: tool.map(Into::into),
            tool_call_id: None,
            tokens_in: None,
            tokens_out: None,
            reasoning_tokens: None,
            cost_usd_e6: None,
            payload: json!({}),
        }
    }

    fn session(start: u64, end: Option<u64>) -> SessionRecord {
        SessionRecord {
            id: "s".into(),
            agent: "cursor".into(),
            model: None,
            workspace: "/ws".into(),
            started_at_ms: start,
            ended_at_ms: end,
            status: SessionStatus::Done,
            trace_path: String::new(),
            start_commit: None,
            end_commit: None,
            branch: None,
            dirty_start: None,
            dirty_end: None,
            repo_binding_source: None,
        }
    }

    #[test]
    fn tokens_sums_all_buckets() {
        let mut e = ev(EventKind::ToolCall, None);
        e.tokens_in = Some(10);
        e.tokens_out = Some(20);
        e.reasoning_tokens = Some(5);
        assert_eq!(tokens(&[e]), 35.0);
    }

    #[test]
    fn success_rate_zero_on_error() {
        let events = vec![ev(EventKind::Message, None), ev(EventKind::Error, None)];
        assert_eq!(success_rate(&events), 0.0);
    }

    #[test]
    fn tool_loops_counts_consecutive_repeats() {
        let events = vec![
            ev(EventKind::ToolCall, Some("read")),
            ev(EventKind::ToolCall, Some("read")),
            ev(EventKind::ToolCall, Some("write")),
            ev(EventKind::ToolCall, Some("read")),
            ev(EventKind::ToolCall, Some("read")),
            ev(EventKind::ToolCall, Some("read")),
        ];
        assert_eq!(tool_loops(&events), 3.0);
    }

    #[test]
    fn duration_requires_end() {
        assert!(duration_minutes(&session(0, None)).is_none());
        assert_eq!(duration_minutes(&session(0, Some(120_000))), Some(2.0));
    }

    #[test]
    fn files_dedup_by_path() {
        let mut a = ev(EventKind::ToolCall, None);
        a.payload = json!({"input":{"path":"src/a.rs"}});
        let mut b = ev(EventKind::ToolCall, None);
        b.payload = json!({"input":{"path":"src/a.rs"}});
        let mut c = ev(EventKind::ToolCall, None);
        c.payload = json!({"path":"src/b.rs"});
        assert_eq!(files_touched(&[a, b, c]), 2.0);
    }
}
