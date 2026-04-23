//! Typed outbound JSON matching `POST /v1/events` (single-event shape used in outbox rows).

use crate::core::event::{Event, EventKind, EventSource, SessionRecord};
use blake3::Hasher;
use serde::{Deserialize, Serialize};
use std::path::Path;

const BLAKE3_PREFIX: &str = "blake3:";

/// Full batch body for `POST /v1/events`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventsBatchBody {
    pub team_id: String,
    pub workspace_hash: String,
    pub events: Vec<OutboundEvent>,
}

/// One event in the ingest API shape (after redaction).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboundEvent {
    pub session_id_hash: String,
    pub event_seq: u64,
    pub ts_ms: u64,
    pub agent: String,
    pub model: String,
    pub kind: String,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens_in: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens_out: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_usd_e6: Option<i64>,
    pub payload: serde_json::Value,
}

pub fn hash_with_salt(team_salt: &[u8; 32], material: &[u8]) -> String {
    let mut h = Hasher::new();
    h.update(team_salt);
    h.update(material);
    format!("{BLAKE3_PREFIX}{}", hex::encode(h.finalize().as_bytes()))
}

pub fn workspace_hash(team_salt: &[u8; 32], workspace_abs: &Path) -> String {
    let normalized = workspace_abs.to_string_lossy();
    hash_with_salt(team_salt, normalized.as_bytes())
}

pub fn outbound_event_from_row(
    e: &Event,
    session: &SessionRecord,
    team_salt: &[u8; 32],
) -> OutboundEvent {
    OutboundEvent {
        session_id_hash: hash_with_salt(team_salt, e.session_id.as_bytes()),
        event_seq: e.seq,
        ts_ms: e.ts_ms,
        agent: session.agent.clone(),
        model: session
            .model
            .clone()
            .unwrap_or_else(|| "unknown".to_string()),
        kind: kind_api(&e.kind),
        source: source_api(&e.source),
        tool: e.tool.clone(),
        tokens_in: e.tokens_in,
        tokens_out: e.tokens_out,
        cost_usd_e6: e.cost_usd_e6,
        payload: e.payload.clone(),
    }
}

fn kind_api(k: &EventKind) -> String {
    match k {
        EventKind::ToolCall => "tool_call",
        EventKind::ToolResult => "tool_result",
        EventKind::Message => "message",
        EventKind::Error => "error",
        EventKind::Cost => "cost",
        EventKind::Hook => "hook",
    }
    .to_string()
}

fn source_api(s: &EventSource) -> String {
    match s {
        EventSource::Tail => "tail",
        EventSource::Hook => "hook",
        EventSource::Proxy => "proxy",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn workspace_hash_stable_for_same_salt_and_path() {
        let salt = [7u8; 32];
        let p = Path::new("/tmp/ws");
        let a = workspace_hash(&salt, p);
        let b = workspace_hash(&salt, p);
        assert_eq!(a, b);
        assert!(a.starts_with(BLAKE3_PREFIX));
    }

    #[test]
    fn outbound_maps_kind_snake_case() {
        let salt = [0u8; 32];
        let session = SessionRecord {
            id: "sid".into(),
            agent: "cursor".into(),
            model: Some("m1".into()),
            workspace: "/w".into(),
            started_at_ms: 0,
            ended_at_ms: None,
            status: crate::core::event::SessionStatus::Running,
            trace_path: "".into(),
        };
        let ev = Event {
            session_id: "sid".into(),
            seq: 3,
            ts_ms: 99,
            kind: EventKind::ToolCall,
            source: EventSource::Hook,
            tool: Some("Edit".into()),
            tokens_in: None,
            tokens_out: None,
            cost_usd_e6: None,
            payload: json!({}),
        };
        let o = outbound_event_from_row(&ev, &session, &salt);
        assert_eq!(o.kind, "tool_call");
        assert_eq!(o.source, "hook");
        assert_eq!(o.event_seq, 3);
    }
}
