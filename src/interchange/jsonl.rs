// SPDX-License-Identifier: AGPL-3.0-or-later
//! Generic JSONL event parser. No filesystem access.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsonlEvent {
    pub line: usize,
    pub session_id: String,
    pub seq: u64,
    pub ts_ms: u64,
    pub kind: String,
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens_in: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens_out: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_usd_e6: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_creation_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_read_tokens: Option<u32>,
    #[serde(default)]
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonlParseError {
    pub line: usize,
    pub message: String,
}

pub fn parse_jsonl_events(input: &str) -> Result<Vec<JsonlEvent>, JsonlParseError> {
    input
        .lines()
        .enumerate()
        .filter_map(|(idx, line)| parse_jsonl_line(idx + 1, line).transpose())
        .collect()
}

pub fn parse_jsonl_line(line: usize, raw: &str) -> Result<Option<JsonlEvent>, JsonlParseError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let value = parse_value(line, trimmed)?;
    Ok(Some(parse_jsonl_value(line, value)?))
}

pub fn parse_jsonl_value(line: usize, value: Value) -> Result<JsonlEvent, JsonlParseError> {
    let payload = value.get("event").cloned().unwrap_or(value);
    event_from_value(line, payload)
}

fn event_from_value(line: usize, value: Value) -> Result<JsonlEvent, JsonlParseError> {
    Ok(JsonlEvent {
        line,
        session_id: req_str(&value, line, &["session_id"], "session_id")?,
        seq: req_u64(&value, line, &["seq", "sequence", "event_seq"], "seq")?,
        ts_ms: req_u64(&value, line, &["ts_ms", "timestamp_ms"], "ts_ms")?,
        kind: req_str(&value, line, &["kind", "type", "event_type"], "kind")?,
        source: opt_str(&value, &["source"]).unwrap_or_else(|| "jsonl".into()),
        tool: opt_str(&value, &["tool", "tool_name"]),
        tool_call_id: opt_str(&value, &["tool_call_id", "call_id"]),
        tokens_in: opt_u32(&value, &["tokens_in", "input_tokens"]),
        tokens_out: opt_u32(&value, &["tokens_out", "output_tokens"]),
        reasoning_tokens: opt_u32(&value, &["reasoning_tokens"]),
        cost_usd_e6: opt_i64(&value, &["cost_usd_e6"]),
        cache_creation_tokens: opt_u32(&value, &["cache_creation_tokens"]),
        cache_read_tokens: opt_u32(&value, &["cache_read_tokens"]),
        payload: payload(&value),
    })
}

fn parse_value(line: usize, raw: &str) -> Result<Value, JsonlParseError> {
    serde_json::from_str(raw).map_err(|err| JsonlParseError {
        line,
        message: err.to_string(),
    })
}

fn req_str(v: &Value, line: usize, keys: &[&str], name: &str) -> Result<String, JsonlParseError> {
    opt_str(v, keys).ok_or_else(|| JsonlParseError::missing(line, name))
}

fn req_u64(v: &Value, line: usize, keys: &[&str], name: &str) -> Result<u64, JsonlParseError> {
    field(v, keys)
        .and_then(Value::as_u64)
        .ok_or_else(|| JsonlParseError::missing(line, name))
}

fn opt_str(v: &Value, keys: &[&str]) -> Option<String> {
    field(v, keys)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

fn opt_u32(v: &Value, keys: &[&str]) -> Option<u32> {
    field(v, keys)?.as_u64()?.try_into().ok()
}

fn opt_i64(v: &Value, keys: &[&str]) -> Option<i64> {
    field(v, keys)?.as_i64()
}

fn field<'a>(v: &'a Value, keys: &[&str]) -> Option<&'a Value> {
    keys.iter().find_map(|key| v.get(*key))
}

fn payload(v: &Value) -> Value {
    v.get("payload").cloned().unwrap_or_else(|| v.clone())
}

impl JsonlParseError {
    fn missing(line: usize, name: &str) -> Self {
        Self {
            line,
            message: format!("missing JSONL event field: {name}"),
        }
    }
}

impl Display for JsonlParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl Error for JsonlParseError {}
