// SPDX-License-Identifier: AGPL-3.0-or-later
//! Pure ATIF mapping types. No storage or CLI wiring.

mod error;
mod types;

use super::jsonl::JsonlEvent;
pub use error::AtifImportError;
use std::collections::BTreeMap;
pub use types::{
    AtifDocument, AtifEvent, AtifSession, InterchangeEvent, InterchangeSession, InterchangeTrace,
};

pub const ATIF_FORMAT: &str = "atif";
pub const ATIF_VERSION: u16 = 1;

pub fn export_atif(trace: &InterchangeTrace) -> AtifDocument {
    AtifDocument {
        format: ATIF_FORMAT.into(),
        version: ATIF_VERSION,
        session: atif_session(&trace.session),
        events: trace.events.iter().map(atif_event).collect(),
    }
}

pub fn import_atif(doc: &AtifDocument) -> Result<InterchangeTrace, AtifImportError> {
    validate_document(doc)?;
    Ok(InterchangeTrace {
        session: interchange_session(&doc.session),
        events: doc
            .events
            .iter()
            .map(|event| interchange_event(&doc.session.id, event))
            .collect(),
    })
}

pub fn trace_from_jsonl(session: InterchangeSession, events: Vec<JsonlEvent>) -> InterchangeTrace {
    InterchangeTrace {
        session,
        events: events.into_iter().map(InterchangeEvent::from).collect(),
    }
}

impl From<JsonlEvent> for InterchangeEvent {
    fn from(event: JsonlEvent) -> Self {
        Self {
            session_id: event.session_id,
            seq: event.seq,
            ts_ms: event.ts_ms,
            kind: event.kind,
            source: event.source,
            tool: event.tool,
            tool_call_id: event.tool_call_id,
            payload: event.payload,
            attributes: BTreeMap::new(),
        }
    }
}

fn validate_document(doc: &AtifDocument) -> Result<(), AtifImportError> {
    validate_header(doc)?;
    doc.events
        .iter()
        .find(|event| !event.id.starts_with(&format!("{}:", doc.session.id)))
        .map_or(Ok(()), |event| Err(mismatch(&event.id, &doc.session.id)))
}

fn validate_header(doc: &AtifDocument) -> Result<(), AtifImportError> {
    if doc.format != ATIF_FORMAT {
        return Err(AtifImportError::UnsupportedFormat(doc.format.clone()));
    }
    (doc.version == ATIF_VERSION)
        .then_some(())
        .ok_or(AtifImportError::UnsupportedVersion(doc.version))
}

fn atif_session(session: &InterchangeSession) -> AtifSession {
    AtifSession {
        id: session.id.clone(),
        agent: session.agent.clone(),
        model: session.model.clone(),
        workspace: session.workspace.clone(),
        started_at_ms: session.started_at_ms,
        ended_at_ms: session.ended_at_ms,
        attributes: session.attributes.clone(),
    }
}

fn interchange_session(session: &AtifSession) -> InterchangeSession {
    InterchangeSession {
        id: session.id.clone(),
        agent: session.agent.clone(),
        model: session.model.clone(),
        workspace: session.workspace.clone(),
        started_at_ms: session.started_at_ms,
        ended_at_ms: session.ended_at_ms,
        attributes: session.attributes.clone(),
    }
}

fn atif_event(event: &InterchangeEvent) -> AtifEvent {
    AtifEvent {
        id: event_id(&event.session_id, event.seq),
        sequence: event.seq,
        timestamp_ms: event.ts_ms,
        event_type: event.kind.clone(),
        source: event.source.clone(),
        tool: event.tool.clone(),
        tool_call_id: event.tool_call_id.clone(),
        payload: event.payload.clone(),
        attributes: event.attributes.clone(),
    }
}

fn interchange_event(session_id: &str, event: &AtifEvent) -> InterchangeEvent {
    InterchangeEvent {
        session_id: session_id.into(),
        seq: event.sequence,
        ts_ms: event.timestamp_ms,
        kind: event.event_type.clone(),
        source: event.source.clone(),
        tool: event.tool.clone(),
        tool_call_id: event.tool_call_id.clone(),
        payload: event.payload.clone(),
        attributes: event.attributes.clone(),
    }
}

fn event_id(session_id: &str, seq: u64) -> String {
    format!("{session_id}:{seq}")
}

fn mismatch(event_id: &str, session_id: &str) -> AtifImportError {
    AtifImportError::SessionMismatch {
        event_id: event_id.into(),
        session_id: session_id.into(),
    }
}
