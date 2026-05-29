// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::core::event::{Event, EventKind, EventSource, SessionRecord};
use crate::store::Store;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AtifDocument {
    pub schema_version: String,
    pub trajectory_id: String,
    pub session: SessionRecord,
    pub steps: Vec<AtifStep>,
    pub metadata: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AtifStep {
    pub seq: u64,
    pub ts_ms: u64,
    pub kind: String,
    pub source: String,
    pub tool: Option<String>,
    pub tokens: AtifTokens,
    pub cost_usd_e6: Option<i64>,
    pub payload: Value,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AtifTokens {
    pub input: Option<u32>,
    pub output: Option<u32>,
    pub reasoning: Option<u32>,
    pub cache_read: Option<u32>,
    pub cache_create: Option<u32>,
}

pub fn export_session(store: &Store, session_id: &str) -> Result<AtifDocument> {
    let session = store
        .get_session(session_id)?
        .with_context(|| format!("session not found: {session_id}"))?;
    let steps = store
        .list_events_for_session(session_id)?
        .into_iter()
        .map(step_from_event)
        .collect();
    Ok(AtifDocument {
        schema_version: "kaizen.atif.v1".into(),
        trajectory_id: session_id.to_string(),
        session,
        steps,
        metadata: json!({"producer":"kaizen"}),
    })
}

pub fn import_file(store: &Store, path: &Path, workspace: &str) -> Result<AtifDocument> {
    let text = std::fs::read_to_string(path)?;
    let mut doc: AtifDocument = serde_json::from_str(&text)?;
    import_document(store, &mut doc, workspace)?;
    Ok(doc)
}

pub fn import_document(store: &Store, doc: &mut AtifDocument, workspace: &str) -> Result<()> {
    doc.session.workspace = workspace.to_string();
    store.upsert_session(&doc.session)?;
    doc.steps
        .iter()
        .map(|step| event_from_step(&doc.session.id, step))
        .try_for_each(|event| store.append_event(&event?))
}

fn step_from_event(event: Event) -> AtifStep {
    let tokens = tokens(&event);
    AtifStep {
        seq: event.seq,
        ts_ms: event.ts_ms,
        kind: format!("{:?}", event.kind),
        source: format!("{:?}", event.source),
        tool: event.tool,
        tokens,
        cost_usd_e6: event.cost_usd_e6,
        payload: event.payload,
    }
}

fn tokens(event: &Event) -> AtifTokens {
    AtifTokens {
        input: event.tokens_in,
        output: event.tokens_out,
        reasoning: event.reasoning_tokens,
        cache_read: event.cache_read_tokens,
        cache_create: event.cache_creation_tokens,
    }
}

fn event_from_step(session_id: &str, step: &AtifStep) -> Result<Event> {
    Ok(Event {
        session_id: session_id.to_string(),
        seq: step.seq,
        ts_ms: step.ts_ms,
        ts_exact: true,
        kind: kind(&step.kind),
        source: source(&step.source),
        tool: step.tool.clone(),
        tool_call_id: None,
        tokens_in: step.tokens.input,
        tokens_out: step.tokens.output,
        reasoning_tokens: step.tokens.reasoning,
        cost_usd_e6: step.cost_usd_e6,
        stop_reason: None,
        latency_ms: None,
        ttft_ms: None,
        retry_count: None,
        context_used_tokens: None,
        context_max_tokens: None,
        cache_creation_tokens: step.tokens.cache_create,
        cache_read_tokens: step.tokens.cache_read,
        system_prompt_tokens: None,
        payload: step.payload.clone(),
    })
}

fn kind(value: &str) -> EventKind {
    match value {
        "ToolCall" => EventKind::ToolCall,
        "ToolResult" => EventKind::ToolResult,
        "Error" => EventKind::Error,
        "Cost" => EventKind::Cost,
        "Hook" => EventKind::Hook,
        "Lifecycle" => EventKind::Lifecycle,
        _ => EventKind::Message,
    }
}

fn source(value: &str) -> EventSource {
    match value {
        "Hook" => EventSource::Hook,
        "Proxy" => EventSource::Proxy,
        _ => EventSource::Tail,
    }
}
