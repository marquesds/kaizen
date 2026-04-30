// SPDX-License-Identifier: AGPL-3.0-or-later
//! Event-to-search-doc projection.

use crate::core::event::{Event, EventKind, SessionRecord};
use crate::store::event_index::{paths_from_event_payload, skills_from_event_json};
use crate::sync::redact::{redact_payload, redact_string};
use serde_json::Value;
use std::path::Path;

#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchDoc {
    pub session_id: String,
    pub seq: u64,
    pub ts_ms: u64,
    pub agent: String,
    pub kind: String,
    pub text: String,
    pub paths: Vec<String>,
    pub skills: Vec<String>,
    pub tokens_total: i64,
}

pub fn extract_doc(
    event: &Event,
    session: &SessionRecord,
    workspace: &Path,
    salt: &[u8; 32],
) -> Option<SearchDoc> {
    let kind = kind_label(&event.kind)?.to_string();
    let mut payload = event.payload.clone();
    redact_payload(&mut payload, workspace, salt);
    let text = event_text(event, &payload, workspace, salt);
    (!text.trim().is_empty()).then(|| SearchDoc {
        session_id: event.session_id.clone(),
        seq: event.seq,
        ts_ms: event.ts_ms,
        agent: session.agent.clone(),
        kind,
        text,
        paths: paths_from_event_payload(&payload),
        skills: skills_from_event_json(&payload),
        tokens_total: tokens_total(event),
    })
}

pub fn kind_label(kind: &EventKind) -> Option<&'static str> {
    match kind {
        EventKind::Message => Some("message"),
        EventKind::ToolCall => Some("tool_use"),
        EventKind::ToolResult => Some("tool_result"),
        _ => None,
    }
}

pub fn tokens_total(event: &Event) -> i64 {
    [event.tokens_in, event.tokens_out, event.reasoning_tokens]
        .into_iter()
        .flatten()
        .map(i64::from)
        .sum()
}

pub fn redacted_event_text(event: &Event, workspace: &Path, salt: &[u8; 32]) -> String {
    let mut payload = event.payload.clone();
    redact_payload(&mut payload, workspace, salt);
    event_text(event, &payload, workspace, salt)
}

pub fn snippet(text: &str, query: &str) -> String {
    let base = text
        .split_whitespace()
        .take(32)
        .collect::<Vec<_>>()
        .join(" ");
    highlight_terms(&base.replace('\n', " "), query)
}

fn event_text(event: &Event, payload: &Value, workspace: &Path, salt: &[u8; 32]) -> String {
    let mut out = Vec::new();
    if let Some(tool) = event.tool.as_deref() {
        out.push(redact_string(tool, workspace, salt));
    }
    collect_strings(payload, &mut out);
    out.join(" ")
}

fn collect_strings(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::String(s) => out.push(s.clone()),
        Value::Array(items) => items.iter().for_each(|v| collect_strings(v, out)),
        Value::Object(map) => map.values().for_each(|v| collect_strings(v, out)),
        _ => {}
    }
}

fn highlight_terms(text: &str, query: &str) -> String {
    query_terms(query)
        .into_iter()
        .fold(text.to_string(), |acc, term| {
            acc.replace(&term, &format!("**{term}**"))
        })
}

fn query_terms(query: &str) -> Vec<String> {
    query
        .split(|c: char| c.is_whitespace() || "():'\"><=".contains(c))
        .filter(|s| s.len() > 2 && !matches!(*s, "AND" | "OR" | "NOT"))
        .map(str::to_string)
        .collect()
}
