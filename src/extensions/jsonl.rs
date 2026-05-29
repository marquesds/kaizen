// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::core::event::{Event, EventKind, EventSource, SessionRecord, SessionStatus};
use crate::interchange::jsonl::{JsonlEvent, parse_jsonl_value};
use crate::store::Store;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct JsonlImportReport {
    pub imported_events: u64,
    pub sessions_created: u64,
}

pub fn import_file(store: &Store, path: &Path, workspace: &str) -> Result<JsonlImportReport> {
    let text = std::fs::read_to_string(path)?;
    text.lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .try_fold(JsonlImportReport::default(), |mut report, (idx, line)| {
            import_line(store, workspace, idx + 1, line, &mut report)?;
            Ok(report)
        })
}

fn import_line(
    store: &Store,
    workspace: &str,
    line_no: usize,
    line: &str,
    report: &mut JsonlImportReport,
) -> Result<()> {
    let event = parse_event(line).with_context(|| format!("line {line_no}"))?;
    ensure_session(store, workspace, &event, report)?;
    store.append_event(&event)?;
    report.imported_events += 1;
    Ok(())
}

fn parse_event(line: &str) -> Result<Event> {
    let value: Value = serde_json::from_str(line)?;
    serde_json::from_value(value.clone()).or_else(|_| generic_event(value))
}

fn generic_event(value: Value) -> Result<Event> {
    serde_json::from_value(value.get("event").cloned().unwrap_or_else(|| value.clone())).or_else(
        |_| {
            parse_jsonl_value(0, value)
                .map(event_from_generic)
                .map_err(Into::into)
        },
    )
}

fn event_from_generic(row: JsonlEvent) -> Event {
    Event {
        session_id: row.session_id,
        seq: row.seq,
        ts_ms: row.ts_ms,
        ts_exact: true,
        kind: kind(&row.kind),
        source: source(&row.source),
        tool: row.tool,
        tool_call_id: row.tool_call_id,
        tokens_in: row.tokens_in,
        tokens_out: row.tokens_out,
        reasoning_tokens: row.reasoning_tokens,
        cost_usd_e6: row.cost_usd_e6,
        stop_reason: None,
        latency_ms: None,
        ttft_ms: None,
        retry_count: None,
        context_used_tokens: None,
        context_max_tokens: None,
        cache_creation_tokens: row.cache_creation_tokens,
        cache_read_tokens: row.cache_read_tokens,
        system_prompt_tokens: None,
        payload: row.payload,
    }
}

fn kind(value: &str) -> EventKind {
    match value.to_ascii_lowercase().replace('-', "_").as_str() {
        "tool_call" | "toolcall" => EventKind::ToolCall,
        "tool_result" | "toolresult" => EventKind::ToolResult,
        "error" => EventKind::Error,
        "cost" => EventKind::Cost,
        "hook" => EventKind::Hook,
        "lifecycle" => EventKind::Lifecycle,
        _ => EventKind::Message,
    }
}

fn source(value: &str) -> EventSource {
    match value.to_ascii_lowercase().as_str() {
        "hook" => EventSource::Hook,
        "proxy" => EventSource::Proxy,
        _ => EventSource::Tail,
    }
}

fn ensure_session(
    store: &Store,
    workspace: &str,
    event: &Event,
    report: &mut JsonlImportReport,
) -> Result<()> {
    if store.get_session(&event.session_id)?.is_some() {
        return Ok(());
    }
    store.upsert_session(&default_session(event, workspace))?;
    report.sessions_created += 1;
    Ok(())
}

fn default_session(event: &Event, workspace: &str) -> SessionRecord {
    SessionRecord {
        id: event.session_id.clone(),
        agent: "jsonl".into(),
        model: None,
        workspace: workspace.into(),
        started_at_ms: event.ts_ms,
        ended_at_ms: None,
        status: SessionStatus::Running,
        trace_path: "jsonl".into(),
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
