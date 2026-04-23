// SPDX-License-Identifier: AGPL-3.0-or-later
//! Parse Cursor hook JSON from stdin.
//!
//! Cursor sends a JSON object with `event`, `session_id`, `timestamp_ms`.
//! Fields may vary by hook type; unknown fields are stored in `payload`.

use super::{EventKind, HookEvent};
use anyhow::{Context, Result, bail};
use serde_json::Value;

/// Parse a Cursor hook payload (one JSON object, UTF-8 string).
///
/// # Errors
/// Returns `Err` if input is not valid JSON or missing required fields.
pub fn parse_cursor_hook(input: &str) -> Result<HookEvent> {
    let v: Value = serde_json::from_str(input.trim()).context("cursor hook: invalid JSON")?;
    let obj = v.as_object().context("cursor hook: expected JSON object")?;

    let kind_str = obj
        .get("event")
        .and_then(|v| v.as_str())
        .context("cursor hook: missing 'event' field")?;

    let session_id = obj
        .get("session_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let ts_ms = obj
        .get("timestamp_ms")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    if session_id.is_empty() {
        bail!("cursor hook: missing 'session_id' field");
    }

    Ok(HookEvent {
        kind: EventKind::parse(kind_str),
        session_id,
        ts_ms,
        payload: Value::Object(obj.clone()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_stop_fixture() {
        let json = include_str!("../../../tests/fixtures/hooks/cursor_stop.json");
        let ev = parse_cursor_hook(json).unwrap();
        assert_eq!(ev.kind, EventKind::Stop);
        assert!(!ev.session_id.is_empty());
    }

    #[test]
    fn missing_event_field_errors() {
        let err = parse_cursor_hook(r#"{"session_id":"s1","timestamp_ms":0}"#);
        assert!(err.is_err());
    }
}
