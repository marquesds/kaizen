// SPDX-License-Identifier: AGPL-3.0-or-later
//! Parse Mistral Vibe hook JSON from stdin.
//!
//! Mistral Vibe sends a JSON object with `event`, `session_id`, `timestamp_ms`.
//! Unknown fields are stored in `payload`.

use super::{EventKind, HookEvent};
use anyhow::{Context, Result, bail};
use serde_json::Value;

/// Map Mistral Vibe event type strings to normalized `EventKind`.
fn vibe_event_kind(s: &str) -> EventKind {
    match s {
        "session_start" => EventKind::SessionStart,
        "session_end" | "stop" => EventKind::Stop,
        "tool_start" | "function_call_start" => EventKind::PreToolUse,
        "tool_end" | "function_call_end" => EventKind::PostToolUse,
        other => EventKind::parse(other),
    }
}

/// Parse a Mistral Vibe hook payload (one JSON object, UTF-8 string).
///
/// # Errors
/// Returns `Err` when input is invalid JSON, not an object, or missing required fields.
pub fn parse_vibe_hook(input: &str) -> Result<HookEvent> {
    let v: Value = serde_json::from_str(input.trim()).context("vibe hook: invalid JSON")?;
    let obj = v.as_object().context("vibe hook: expected JSON object")?;

    let kind_str = obj
        .get("event")
        .and_then(|v| v.as_str())
        .context("vibe hook: missing 'event' field")?;

    let session_id = obj
        .get("session_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    if session_id.is_empty() {
        bail!("vibe hook: missing 'session_id' field");
    }

    let ts_ms = obj
        .get("timestamp_ms")
        .and_then(|v| v.as_u64())
        .or_else(|| {
            obj.get("timestamp").and_then(|v| v.as_u64()).map(|t| {
                if t < 1_000_000_000_000 {
                    t.saturating_mul(1000)
                } else {
                    t
                }
            })
        })
        .unwrap_or(0);

    Ok(HookEvent {
        kind: vibe_event_kind(kind_str),
        session_id,
        ts_ms,
        payload: Value::Object(obj.clone()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_session_start() {
        let json =
            r#"{"event":"session_start","session_id":"vibe-sess-1","timestamp_ms":1714000000000}"#;
        let ev = parse_vibe_hook(json).unwrap();
        assert_eq!(ev.kind, EventKind::SessionStart);
        assert_eq!(ev.session_id, "vibe-sess-1");
        assert_eq!(ev.ts_ms, 1714000000000);
    }

    #[test]
    fn parse_session_end() {
        let json =
            r#"{"event":"session_end","session_id":"vibe-sess-2","timestamp_ms":1714000001000}"#;
        let ev = parse_vibe_hook(json).unwrap();
        assert_eq!(ev.kind, EventKind::Stop);
        assert_eq!(ev.session_id, "vibe-sess-2");
    }

    #[test]
    fn parse_tool_start() {
        let json = r#"{"event":"tool_start","session_id":"vibe-sess-3","timestamp_ms":0}"#;
        let ev = parse_vibe_hook(json).unwrap();
        assert_eq!(ev.kind, EventKind::PreToolUse);
    }

    #[test]
    fn parse_tool_end() {
        let json = r#"{"event":"tool_end","session_id":"vibe-sess-4","timestamp_ms":0}"#;
        let ev = parse_vibe_hook(json).unwrap();
        assert_eq!(ev.kind, EventKind::PostToolUse);
    }

    #[test]
    fn missing_session_id_errors() {
        let err = parse_vibe_hook(r#"{"event":"session_start","timestamp_ms":0}"#);
        assert!(err.is_err());
    }

    #[test]
    fn missing_event_field_errors() {
        let err = parse_vibe_hook(r#"{"session_id":"s1","timestamp_ms":0}"#);
        assert!(err.is_err());
    }

    #[test]
    fn timestamp_fallback() {
        // Unix timestamp in seconds should be converted to ms
        let json = r#"{"event":"session_start","session_id":"s1","timestamp":1714000000}"#;
        let ev = parse_vibe_hook(json).unwrap();
        assert_eq!(ev.ts_ms, 1714000000000);
    }
}
