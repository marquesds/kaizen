// SPDX-License-Identifier: AGPL-3.0-or-later

use serde_json::Value;
use std::path::Path;

pub(crate) type Usage = (Option<u32>, Option<u32>, Option<u32>);

pub(crate) fn json(line: &str) -> Option<Value> {
    serde_json::from_str(line.trim()).ok()
}

pub(crate) fn session_id(v: &Value) -> Option<String> {
    text(v, "session_id")
        .or_else(|| text(v, "sessionId"))
        .or_else(|| session_marker(v).then(|| text(v, "id")).flatten())
}

pub(crate) fn workspace(v: &Value) -> Option<String> {
    [
        "cwd",
        "workspace",
        "workspacePath",
        "projectPath",
        "project_path",
        "root",
    ]
    .iter()
    .find_map(|k| text(v, k))
    .or_else(|| v.get("payload").and_then(workspace))
}

pub(crate) fn usage(v: &Value) -> Usage {
    let u = v
        .get("usage")
        .or_else(|| v.get("usageMetadata"))
        .unwrap_or(v);
    (
        u32_any(u, &["input_tokens", "prompt_tokens", "promptTokenCount"]),
        u32_any(
            u,
            &["output_tokens", "completion_tokens", "candidatesTokenCount"],
        ),
        u32_any(u, &["reasoning_tokens", "thoughtsTokenCount"]),
    )
}

pub(crate) fn timestamp(v: &Value) -> Option<u64> {
    [
        "timestamp_ms",
        "ts_ms",
        "created_at_ms",
        "timestamp",
        "created_at",
    ]
    .iter()
    .find_map(|k| v.get(*k).and_then(super::value_ts_ms))
    .or_else(|| v.pointer("/payload/timestamp").and_then(super::value_ts_ms))
}

pub(crate) fn tool(v: &Value) -> Option<String> {
    ["name", "tool", "tool_name", "toolName"]
        .iter()
        .find_map(|k| text(v, k))
}

pub(crate) fn call_id(v: &Value) -> Option<String> {
    ["tool_call_id", "tool_use_id", "call_id", "id"]
        .iter()
        .find_map(|k| text(v, k))
}

pub(crate) fn text(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(ToOwned::to_owned)
}

pub(crate) fn file_stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string()
}

pub(crate) fn file_mtime_ms(path: &Path) -> u64 {
    path.metadata()
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn session_marker(v: &Value) -> bool {
    matches!(
        text(v, "type").or_else(|| text(v, "event")).as_deref(),
        Some("session")
    )
}

fn u32_any(v: &Value, keys: &[&str]) -> Option<u32> {
    keys.iter()
        .find_map(|k| v.get(*k)?.as_u64()?.try_into().ok())
}
