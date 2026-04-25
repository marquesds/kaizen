// SPDX-License-Identifier: AGPL-3.0-or-later
//! Parse OpenClaw hook JSON posted by the `kaizen-events` TS handler.
//!
//! The TS handler serialises the raw OpenClaw event plus these added fields:
//! `event` (type string), `session_id` (required), `timestamp_ms` (u64).

use super::{EventKind, HookEvent};
use anyhow::{Context, Result, bail};
use serde_json::Value;

/// Map OpenClaw event type strings to normalized `EventKind`.
fn openclaw_event_kind(s: &str) -> EventKind {
    match s {
        "command:new" | "message:received" => EventKind::SessionStart,
        "command:stop" => EventKind::Stop,
        "message:sent" | "command:reset" => EventKind::PreToolUse,
        other => EventKind::parse(other),
    }
}

/// Parse an OpenClaw hook payload (one JSON object per call from the TS handler).
///
/// # Errors
/// Returns `Err` when input is invalid JSON, not an object, or missing required fields.
pub fn parse_openclaw_hook(input: &str) -> Result<HookEvent> {
    let v: Value = serde_json::from_str(input.trim()).context("openclaw hook: invalid JSON")?;
    let obj = v
        .as_object()
        .context("openclaw hook: expected JSON object")?;

    let kind_str = obj
        .get("event")
        .and_then(|v| v.as_str())
        .context("openclaw hook: missing 'event' field")?;

    let session_id = obj
        .get("session_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    if session_id.is_empty() {
        bail!("openclaw hook: missing 'session_id' field");
    }

    let ts_ms = obj
        .get("timestamp_ms")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    Ok(HookEvent {
        kind: openclaw_event_kind(kind_str),
        session_id,
        ts_ms,
        payload: Value::Object(obj.clone()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_command_stop() {
        let json =
            r#"{"event":"command:stop","session_id":"oc-sess-1","timestamp_ms":1714000000000}"#;
        let ev = parse_openclaw_hook(json).unwrap();
        assert_eq!(ev.kind, EventKind::Stop);
        assert_eq!(ev.session_id, "oc-sess-1");
        assert_eq!(ev.ts_ms, 1714000000000);
    }

    #[test]
    fn parse_command_new_maps_to_session_start() {
        let json = r#"{"event":"command:new","session_id":"oc-sess-2","timestamp_ms":0}"#;
        let ev = parse_openclaw_hook(json).unwrap();
        assert_eq!(ev.kind, EventKind::SessionStart);
    }

    #[test]
    fn missing_session_id_errors() {
        let err = parse_openclaw_hook(r#"{"event":"command:stop"}"#);
        assert!(err.is_err());
    }

    #[test]
    fn missing_event_field_errors() {
        let err = parse_openclaw_hook(r#"{"session_id":"s1","timestamp_ms":0}"#);
        assert!(err.is_err());
    }

    #[test]
    fn unknown_event_stored_as_unknown_kind() {
        let json = r#"{"event":"session:patch","session_id":"s3","timestamp_ms":0}"#;
        let ev = parse_openclaw_hook(json).unwrap();
        assert!(matches!(ev.kind, EventKind::Unknown(_)));
    }
}
