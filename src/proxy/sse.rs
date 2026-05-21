// SPDX-License-Identifier: AGPL-3.0-or-later
//! Find Anthropic `usage` and `stop_reason` in a buffered SSE or JSON body.

use serde_json::Value;
use std::str::from_utf8;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct UsageData {
    pub tokens_in: Option<u32>,
    pub tokens_out: Option<u32>,
    pub reasoning_tokens: Option<u32>,
    pub cache_creation_tokens: Option<u32>,
    pub cache_read_tokens: Option<u32>,
    pub stop_reason: Option<String>,
}

/// Parse usage and stop_reason from a buffered SSE or JSON response body.
pub fn find_usage_in_body(bytes: &[u8], is_sse: bool) -> UsageData {
    if is_sse {
        parse_sse_lines(bytes)
    } else {
        parse_json_body(bytes)
    }
}

fn parse_sse_lines(bytes: &[u8]) -> UsageData {
    let mut out = UsageData::default();
    for line in bytes.split(|&b| b == b'\n') {
        let s = match from_utf8(line) {
            Ok(t) => t.trim(),
            Err(_) => continue,
        };
        let Some(data) = s.strip_prefix("data: ") else {
            continue;
        };
        let Ok(v) = serde_json::from_str::<Value>(data) else {
            continue;
        };
        walk_merge(&mut out, &v, 0);
    }
    out
}

fn parse_json_body(bytes: &[u8]) -> UsageData {
    let Ok(v) = serde_json::from_slice::<Value>(bytes) else {
        return UsageData::default();
    };
    let mut out = UsageData::default();
    walk_merge(&mut out, &v, 0);
    out
}

const MAX_WALK: usize = 32;

/// Recursively scan `v`; merge any `usage` object and `stop_reason` found.
fn walk_merge(out: &mut UsageData, v: &Value, d: usize) {
    if d > MAX_WALK {
        return;
    }
    match v {
        Value::Object(map) => {
            if let Some(u) = map.get("usage") {
                merge_usage_object(out, u);
            }
            merge_stop_reason(out, v);
            for x in map.values() {
                walk_merge(out, x, d + 1);
            }
        }
        Value::Array(a) => {
            for x in a {
                walk_merge(out, x, d + 1);
            }
        }
        _ => {}
    }
}

fn merge_usage_object(out: &mut UsageData, u: &Value) {
    if let Some(n) = key_u32(u, "input_tokens") {
        out.tokens_in = Some(n);
    }
    if let Some(n) = key_u32(u, "prompt_tokens") {
        out.tokens_in = Some(n);
    }
    if let Some(n) = key_u32(u, "output_tokens") {
        out.tokens_out = Some(n);
    }
    if let Some(n) = key_u32(u, "completion_tokens") {
        out.tokens_out = Some(n);
    }
    if let Some(n) = key_u32(u, "cache_creation_input_tokens") {
        out.cache_creation_tokens = Some(n);
    }
    if let Some(n) = key_u32(u, "cache_read_input_tokens") {
        out.cache_read_tokens = Some(n);
    }
    merge_openai_details(out, u);
}

fn merge_openai_details(out: &mut UsageData, u: &Value) {
    if let Some(n) = u
        .get("input_tokens_details")
        .or_else(|| u.get("prompt_tokens_details"))
        .and_then(|d| key_u32(d, "cached_tokens"))
    {
        out.cache_read_tokens = Some(n);
    }
    if let Some(n) = u
        .get("output_tokens_details")
        .or_else(|| u.get("completion_tokens_details"))
        .and_then(|d| key_u32(d, "reasoning_tokens"))
    {
        out.reasoning_tokens = Some(n);
    }
}

fn merge_stop_reason(out: &mut UsageData, v: &Value) {
    if out.stop_reason.is_some() {
        return;
    }
    if let Some(sr) = v
        .get("delta")
        .and_then(|d| d.get("stop_reason"))
        .and_then(|s| s.as_str())
    {
        out.stop_reason = Some(sr.to_string());
        return;
    }
    if let Some(sr) = v.get("stop_reason").and_then(|s| s.as_str()) {
        out.stop_reason = Some(sr.to_string());
        return;
    }
    if let Some(sr) = first_finish_reason(v) {
        out.stop_reason = Some(sr.to_string());
        return;
    }
    if v.get("status").and_then(|s| s.as_str()) == Some("incomplete")
        && let Some(reason) = v
            .get("incomplete_details")
            .and_then(|d| d.get("reason"))
            .and_then(|s| s.as_str())
    {
        out.stop_reason = Some(reason.to_string());
    }
}

fn first_finish_reason(v: &Value) -> Option<&str> {
    v.get("choices")?
        .as_array()?
        .iter()
        .find_map(|c| c.get("finish_reason").and_then(|s| s.as_str()))
}

fn key_u32(u: &Value, k: &str) -> Option<u32> {
    u.get(k).and_then(json_u32)
}

fn json_u32(n: &Value) -> Option<u32> {
    n.as_u64()
        .map(|x| x as u32)
        .or_else(|| n.as_f64().and_then(|f| f.is_finite().then_some(f as u32)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_usage() {
        let s = r#"{"usage":{"input_tokens":3,"output_tokens":5}}"#;
        let u = find_usage_in_body(s.as_bytes(), false);
        assert_eq!(u.tokens_in, Some(3));
        assert_eq!(u.tokens_out, Some(5));
    }

    #[test]
    fn sse_data_line() {
        let s = "data: {\"type\":\"x\",\"usage\":{\"input_tokens\":1,\"output_tokens\":2}}\n\n";
        let u = find_usage_in_body(s.as_bytes(), true);
        assert_eq!(u.tokens_in, Some(1));
        assert_eq!(u.tokens_out, Some(2));
    }

    #[test]
    fn cache_tokens_parsed() {
        let s = r#"{"usage":{"input_tokens":10,"output_tokens":5,
            "cache_creation_input_tokens":100,"cache_read_input_tokens":200}}"#;
        let u = find_usage_in_body(s.as_bytes(), false);
        assert_eq!(u.cache_creation_tokens, Some(100));
        assert_eq!(u.cache_read_tokens, Some(200));
    }

    #[test]
    fn stop_reason_from_message_delta() {
        let s = "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"}}\n\n";
        let u = find_usage_in_body(s.as_bytes(), true);
        assert_eq!(u.stop_reason, Some("end_turn".to_string()));
    }

    #[test]
    fn openai_responses_usage() {
        let s = r#"{"usage":{"input_tokens":75,"output_tokens":1186,
            "input_tokens_details":{"cached_tokens":5},
            "output_tokens_details":{"reasoning_tokens":1024}}}"#;
        let u = find_usage_in_body(s.as_bytes(), false);
        assert_eq!(u.tokens_in, Some(75));
        assert_eq!(u.tokens_out, Some(1186));
        assert_eq!(u.reasoning_tokens, Some(1024));
        assert_eq!(u.cache_read_tokens, Some(5));
    }

    #[test]
    fn openai_chat_usage() {
        let s = r#"{"choices":[{"finish_reason":"stop"}],"usage":{
            "prompt_tokens":1117,"completion_tokens":46,
            "prompt_tokens_details":{"cached_tokens":9},
            "completion_tokens_details":{"reasoning_tokens":2}}}"#;
        let u = find_usage_in_body(s.as_bytes(), false);
        assert_eq!(u.tokens_in, Some(1117));
        assert_eq!(u.tokens_out, Some(46));
        assert_eq!(u.reasoning_tokens, Some(2));
        assert_eq!(u.cache_read_tokens, Some(9));
        assert_eq!(u.stop_reason, Some("stop".to_string()));
    }
}
