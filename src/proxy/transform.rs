// SPDX-License-Identifier: AGPL-3.0-or-later
//! JSON minify and optional `messages` truncation (Anthropic-style bodies).

use crate::core::config::ContextPolicy;
use serde_json::Value;
use std::str::from_utf8;

/// Minify and/or shrink `messages` (when `policy` is not `None`).
pub fn process_request_bytes(
    body: &[u8],
    minify: bool,
    policy: &ContextPolicy,
) -> Result<Vec<u8>, anyhow::Error> {
    if body.is_empty() {
        return Ok(body.to_vec());
    }
    let t = from_utf8(body)?;
    let v: Value = serde_json::from_str(t).map_err(|e| {
        let msg = e.to_string();
        anyhow::anyhow!("json parse: {msg}")
    })?;
    let v = if matches!(policy, ContextPolicy::None) {
        v
    } else {
        apply_context_policy(&v, policy)
    };
    if minify {
        Ok(serde_json::to_vec(&v)?)
    } else {
        Ok(v.to_string().into_bytes())
    }
}

/// Extract `model` for session metadata; never logged to sync in raw payload redaction.
pub fn try_model(maybe_json: &Value) -> Option<String> {
    maybe_json
        .get("model")
        .and_then(|m| m.as_str())
        .map(std::string::ToString::to_string)
}

fn apply_context_policy(input: &Value, policy: &ContextPolicy) -> Value {
    let mut v = input.clone();
    let Some(msgs) = v.get_mut("messages").and_then(|m| m.as_array_mut()) else {
        return v;
    };
    match policy {
        ContextPolicy::None => v,
        ContextPolicy::LastMessages { count } => {
            if *count == 0 || msgs.len() <= *count {
                v
            } else {
                let keep = *count;
                *msgs = msgs[msgs.len() - keep..].to_vec();
                v
            }
        }
        ContextPolicy::MaxInputTokens { max } => {
            if *max == 0 {
                msgs.clear();
                v
            } else {
                while !msgs.is_empty() && est_tokens_for_messages(msgs) > *max {
                    msgs.remove(0);
                }
                v
            }
        }
    }
}

fn est_tokens_for_messages(msgs: &[Value]) -> u32 {
    let ser = serde_json::to_string(msgs).unwrap_or_default();
    (ser.len() as u64 / 4) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn last_messages_keeps_tail() {
        let v = json!({
            "model": "m",
            "messages": [ {"role": "u", "content": "a"},
                          {"role": "a", "content": "b"},
                          {"role": "u", "content": "c"} ]
        });
        let out = apply_context_policy(&v, &ContextPolicy::LastMessages { count: 1 });
        let arr = out["messages"].as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["content"], "c");
    }
}
