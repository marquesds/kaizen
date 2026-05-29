// SPDX-License-Identifier: AGPL-3.0-or-later
use serde_json::Value;
use std::path::{Path, PathBuf};

pub(super) fn workspace_field(value: &Value) -> Option<String> {
    text_any(
        value,
        &["workspace", "cwd", "workspacePath", "projectPath", "root"],
    )
}

pub(super) fn id_field(value: &Value) -> Option<String> {
    text_any(value, &["session_id", "sessionId", "id", "conversationId"])
}

pub(super) fn ts_field(value: &Value) -> Option<u64> {
    ts_any(
        value,
        &["timestamp_ms", "created_at_ms", "createdAt", "timestamp"],
    )
}

pub(super) fn text(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

pub(super) fn ts_any(value: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(super::value_ts_ms))
}

pub(super) fn key_suffix(key: &str) -> String {
    key.rsplit(':').next().unwrap_or(key).to_string()
}

pub(super) fn workspace_matches(candidate: &str, workspace: &Path) -> bool {
    std::fs::canonicalize(candidate)
        .ok()
        .is_some_and(|path| path == canonical(workspace))
}

pub(super) fn file_mtime_ms(path: &Path) -> u64 {
    path.metadata()
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn text_any(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| text(value, key))
}

fn canonical(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}
