// SPDX-License-Identifier: AGPL-3.0-or-later
//! Best-effort LLM model id from transcript JSONL lines or hook payloads.

use serde_json::Value;

fn non_empty(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

/// Read model from a JSON object (transcript line or hook body).
/// Tries `model`, then nested paths used by common agent formats.
pub fn from_object(obj: &serde_json::Map<String, Value>) -> Option<String> {
    if let Some(s) = obj
        .get("model")
        .and_then(|v| v.as_str())
        .and_then(non_empty)
    {
        return Some(s);
    }
    for (parent, key) in [
        ("message", "model"),
        ("metadata", "model"),
        ("config", "model"),
    ] {
        if let Some(s) = obj
            .get(parent)
            .and_then(|o| o.get(key))
            .and_then(|v| v.as_str())
            .and_then(non_empty)
        {
            return Some(s);
        }
    }
    None
}

/// Parse a single JSONL line and return a model id when present.
pub fn from_line(line: &str) -> Option<String> {
    let v: Value = serde_json::from_str(line.trim()).ok()?;
    v.as_object().and_then(from_object)
}

/// Extract model from a JSON value (e.g. hook `payload`).
pub fn from_value(v: &Value) -> Option<String> {
    v.as_object().and_then(from_object)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_system_init() {
        let j = r#"{"type":"system","subtype":"init","session_id":"s1","model":"Claude 4 Sonnet"}"#;
        assert_eq!(from_line(j), Some("Claude 4 Sonnet".into()));
    }

    #[test]
    fn openai_top_level_model() {
        let j = r#"{"model":"gpt-4o","role":"assistant"}"#;
        assert_eq!(from_line(j), Some("gpt-4o".into()));
    }

    #[test]
    fn message_nested_model() {
        let v = serde_json::json!({"message": {"model": "claude-3-5-sonnet-20241022"}});
        assert_eq!(from_value(&v), Some("claude-3-5-sonnet-20241022".into()));
    }

    #[test]
    fn empty_model_ignored() {
        let v = serde_json::json!({"model": "  "});
        assert_eq!(from_value(&v), None);
    }
}
