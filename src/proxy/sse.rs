// SPDX-License-Identifier: AGPL-3.0-or-later
//! Find Anthropic `usage` in a buffered SSE or JSON body.

use serde_json::Value;
use std::str::from_utf8;

type Usage3 = (Option<u32>, Option<u32>, Option<u32>);

/// Returns `(in, out, _reasoning)`; third is reserved for future token kinds.
pub fn find_usage_in_body(bytes: &[u8], is_sse: bool) -> Usage3 {
    if is_sse {
        for line in bytes.split(|&b| b == b'\n') {
            let s = if let Ok(t) = from_utf8(line) {
                t.trim()
            } else {
                continue;
            };
            if let Some(data) = s.strip_prefix("data: ") {
                if let Ok(v) = serde_json::from_str::<Value>(data) {
                    if let Some(t) = extract_usage(&v) {
                        return t;
                    }
                }
            }
        }
    } else if let Ok(v) = serde_json::from_slice::<Value>(bytes) {
        if let Some(t) = extract_usage(&v) {
            return t;
        }
    }
    (None, None, None)
}

fn extract_usage(v: &Value) -> Option<Usage3> {
    v.get("usage")
        .and_then(from_usage_object)
        .or_else(|| walk_usage(v, 0))
}

fn from_usage_object(u: &Value) -> Option<Usage3> {
    let ti = key_u32(u, "input_tokens");
    let to = key_u32(u, "output_tokens");
    (ti.is_some() || to.is_some()).then_some((ti, to, None))
}

fn key_u32(u: &Value, k: &str) -> Option<u32> {
    u.get(k).and_then(json_u32)
}

fn json_u32(n: &Value) -> Option<u32> {
    n.as_u64()
        .map(|x| x as u32)
        .or_else(|| n.as_f64().and_then(|f| f.is_finite().then_some(f as u32)))
}

const MAX_WALK: usize = 32;

fn walk_usage(v: &Value, d: usize) -> Option<Usage3> {
    if d > MAX_WALK {
        return None;
    }
    if let Some(t) = v.get("usage").and_then(from_usage_object) {
        return Some(t);
    }
    match v {
        Value::Object(map) => {
            for x in map.values() {
                if let Some(t) = walk_usage(x, d + 1) {
                    return Some(t);
                }
            }
        }
        Value::Array(a) => {
            for x in a {
                if let Some(t) = walk_usage(x, d + 1) {
                    return Some(t);
                }
            }
        }
        _ => {}
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_usage() {
        let s = r#"{"usage":{"input_tokens":3,"output_tokens":5}}"#;
        let (i, o, r) = find_usage_in_body(s.as_bytes(), false);
        assert_eq!((i, o, r), (Some(3), Some(5), None));
    }

    #[test]
    fn sse_data_line() {
        let s = "data: {\"type\":\"x\",\"usage\":{\"input_tokens\":1,\"output_tokens\":2}}\n\n";
        let (i, o, r) = find_usage_in_body(s.as_bytes(), true);
        assert_eq!((i, o, r), (Some(1), Some(2), None));
    }
}
