// SPDX-License-Identifier: AGPL-3.0-or-later
//! Ingest OpenCode session JSON from `~/.local/share/opencode` (or `OPENCODE_DATA_DIR`).

use crate::collect::model_from_json;
use crate::core::event::{Event, EventKind, EventSource, SessionRecord, SessionStatus};
use anyhow::Result;
use serde_json::Value;
use std::path::{Path, PathBuf};

const AGENT: &str = "opencode";

fn data_dir() -> PathBuf {
    if let Ok(p) = std::env::var("OPENCODE_DATA_DIR") {
        return PathBuf::from(p);
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".local/share/opencode");
    }
    PathBuf::from(".local/share/opencode")
}

fn canonical(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

fn paths_equal(a: &Path, b: &Path) -> bool {
    canonical(a) == canonical(b)
}

/// Session root directory for this workspace (path contains workspace id / folder).
fn session_root_matches_workspace(session_file: &Path, workspace: &Path) -> bool {
    let ws = canonical(workspace);
    let mut cur = session_file.parent();
    let mut depth = 0u8;
    while let Some(p) = cur {
        if depth > 12 {
            break;
        }
        if paths_equal(p, &ws) {
            return true;
        }
        if let Ok(read) = std::fs::read_to_string(p.join("workspace.json")) {
            if workspace_json_folder_matches(&read, workspace) {
                return true;
            }
        }
        cur = p.parent();
        depth += 1;
    }
    false
}

fn path_from_uri_or_path(s: &str) -> PathBuf {
    let p = s.strip_prefix("file://").unwrap_or(s);
    PathBuf::from(p)
}

fn workspace_json_folder_matches(json: &str, workspace: &Path) -> bool {
    let Ok(v) = serde_json::from_str::<Value>(json) else {
        return false;
    };
    let folder = v.get("folder").and_then(|f| f.as_str()).or_else(|| {
        v.get("workspace")
            .and_then(|w| w.get("folder"))
            .and_then(|f| f.as_str())
    });
    let Some(f) = folder else {
        return false;
    };
    paths_equal(&path_from_uri_or_path(f), workspace)
}

fn session_json_directory_field(v: &Value, workspace: &Path) -> bool {
    let ws_str = workspace.to_string_lossy();
    for key in [
        "directory",
        "projectPath",
        "cwd",
        "root",
        "workspacePath",
        "workspaceRoot",
    ] {
        if let Some(s) = v.get(key).and_then(|x| x.as_str()) {
            if paths_equal(Path::new(s), workspace) {
                return true;
            }
            if s == ws_str.as_ref() {
                return true;
            }
        }
    }
    if let Some(folder) = v.get("folder").and_then(|f| f.as_str()) {
        if paths_equal(&path_from_uri_or_path(folder), workspace) {
            return true;
        }
    }
    false
}

fn events_from_messages_array(session_id: &str, messages: &[Value]) -> Vec<Event> {
    let mut events = Vec::new();
    let mut seq: u64 = 0;
    for msg in messages {
        let ts_ms = msg
            .get("time")
            .or_else(|| msg.get("timestamp"))
            .and_then(|t| t.as_u64())
            .or_else(|| {
                msg.get("createdAt")
                    .and_then(|t| t.as_u64())
                    .map(|u| u.saturating_mul(1000))
            })
            .unwrap_or_else(|| seq.saturating_mul(100));

        if let Some(parts) = msg.get("parts").and_then(|p| p.as_array()) {
            for part in parts {
                let typ = part.get("type").and_then(|t| t.as_str()).unwrap_or("");
                match typ {
                    "tool-call" | "tool-invocation" | "tool_call" => {
                        let tool = part
                            .get("toolName")
                            .or_else(|| part.get("tool"))
                            .or_else(|| part.get("name"))
                            .and_then(|x| x.as_str())
                            .unwrap_or("")
                            .to_string();
                        let id = part
                            .get("toolCallId")
                            .or_else(|| part.get("tool_call_id"))
                            .or_else(|| part.get("id"))
                            .and_then(|x| x.as_str())
                            .unwrap_or("")
                            .to_string();
                        events.push(Event {
                            session_id: session_id.to_string(),
                            seq,
                            ts_ms,
                            ts_exact: false,
                            kind: EventKind::ToolCall,
                            source: EventSource::Tail,
                            tool: Some(tool),
                            tool_call_id: Some(id),
                            tokens_in: None,
                            tokens_out: None,
                            reasoning_tokens: None,
                            cost_usd_e6: None,
                            payload: part.clone(),
                        });
                        seq += 1;
                    }
                    "tool-result" | "tool_result" => {
                        let id = part
                            .get("toolCallId")
                            .or_else(|| part.get("tool_call_id"))
                            .and_then(|x| x.as_str())
                            .unwrap_or("")
                            .to_string();
                        events.push(Event {
                            session_id: session_id.to_string(),
                            seq,
                            ts_ms,
                            ts_exact: false,
                            kind: EventKind::ToolResult,
                            source: EventSource::Tail,
                            tool: None,
                            tool_call_id: Some(id),
                            tokens_in: None,
                            tokens_out: None,
                            reasoning_tokens: None,
                            cost_usd_e6: None,
                            payload: part.clone(),
                        });
                        seq += 1;
                    }
                    _ => {}
                }
            }
        }

        if let Some(tc) = msg.get("toolCalls").and_then(|t| t.as_array()) {
            for call in tc {
                let tool = call
                    .get("name")
                    .or_else(|| call.get("function").and_then(|f| f.get("name")))
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                let id = call
                    .get("id")
                    .or_else(|| call.get("toolCallId"))
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                events.push(Event {
                    session_id: session_id.to_string(),
                    seq,
                    ts_ms,
                    ts_exact: false,
                    kind: EventKind::ToolCall,
                    source: EventSource::Tail,
                    tool: Some(tool),
                    tool_call_id: Some(id),
                    tokens_in: None,
                    tokens_out: None,
                    reasoning_tokens: None,
                    cost_usd_e6: None,
                    payload: call.clone(),
                });
                seq += 1;
            }
        }

        if let Some(content) = msg.get("content").and_then(|c| c.as_array()) {
            for block in content {
                let typ = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
                if typ == "tool_use" || typ == "tool-call" {
                    let tool = block
                        .get("name")
                        .and_then(|n| n.as_str())
                        .unwrap_or("")
                        .to_string();
                    let id = block
                        .get("id")
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .to_string();
                    events.push(Event {
                        session_id: session_id.to_string(),
                        seq,
                        ts_ms,
                        ts_exact: false,
                        kind: EventKind::ToolCall,
                        source: EventSource::Tail,
                        tool: Some(tool),
                        tool_call_id: Some(id),
                        tokens_in: None,
                        tokens_out: None,
                        reasoning_tokens: None,
                        cost_usd_e6: None,
                        payload: block.clone(),
                    });
                    seq += 1;
                } else if typ == "tool_result" {
                    let id = block
                        .get("tool_use_id")
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .to_string();
                    events.push(Event {
                        session_id: session_id.to_string(),
                        seq,
                        ts_ms,
                        ts_exact: false,
                        kind: EventKind::ToolResult,
                        source: EventSource::Tail,
                        tool: None,
                        tool_call_id: Some(id),
                        tokens_in: None,
                        tokens_out: None,
                        reasoning_tokens: None,
                        cost_usd_e6: None,
                        payload: block.clone(),
                    });
                    seq += 1;
                }
            }
        }
    }
    events
}

/// Parse one OpenCode session JSON file.
pub fn parse_opencode_session_file(
    path: &Path,
    workspace: &Path,
) -> Result<Option<(SessionRecord, Vec<Event>)>> {
    let text = std::fs::read_to_string(path)?;
    let v: Value = serde_json::from_str(&text)?;
    if !session_json_directory_field(&v, workspace)
        && !session_root_matches_workspace(path, workspace)
    {
        return Ok(None);
    }
    let session_id = v
        .get("id")
        .or_else(|| v.get("sessionId"))
        .and_then(|x| x.as_str())
        .map(ToOwned::to_owned)
        .or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .map(ToOwned::to_owned)
        })
        .unwrap_or_else(|| "opencode-session".to_string());

    let messages = v
        .get("messages")
        .and_then(|m| m.as_array())
        .cloned()
        .unwrap_or_default();
    if messages.is_empty() {
        return Ok(None);
    }

    let model = v
        .get("model")
        .and_then(|m| m.as_str())
        .map(ToOwned::to_owned)
        .or_else(|| model_from_json::from_value(&v));

    let events = events_from_messages_array(&session_id, &messages);
    if events.is_empty() {
        return Ok(None);
    }

    let started_at_ms = events.first().map(|e| e.ts_ms).unwrap_or(0);
    Ok(Some((
        SessionRecord {
            id: session_id,
            agent: AGENT.to_string(),
            model,
            workspace: workspace.to_string_lossy().to_string(),
            started_at_ms,
            ended_at_ms: None,
            status: SessionStatus::Done,
            trace_path: path.to_string_lossy().to_string(),
            start_commit: None,
            end_commit: None,
            branch: None,
            dirty_start: None,
            dirty_end: None,
            repo_binding_source: None,
        },
        events,
    )))
}

fn walk_json_files(dir: &Path, out: &mut Vec<PathBuf>, depth: u8) {
    if depth > 14 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk_json_files(&p, out, depth + 1);
        } else if p.extension().and_then(|x| x.to_str()) == Some("json") {
            if let Ok(m) = p.metadata() {
                if m.len() > 32 {
                    out.push(p);
                }
            }
        }
    }
}

/// Scan default (or `OPENCODE_DATA_DIR`) OpenCode storage for sessions tied to `workspace`.
pub fn scan_opencode_workspace(workspace: &Path) -> Result<Vec<(SessionRecord, Vec<Event>)>> {
    let root = data_dir();
    let project = root.join("project");
    let storage = root.join("storage");
    let mut files = Vec::new();
    let local_opencode = workspace.join(".opencode");
    if local_opencode.is_dir() {
        walk_json_files(&local_opencode, &mut files, 0);
    }
    if project.is_dir() {
        walk_json_files(&project, &mut files, 0);
    }
    if storage.is_dir() {
        walk_json_files(&storage, &mut files, 0);
    }
    let mut sessions = Vec::new();
    for f in files {
        if let Ok(Some(pair)) = parse_opencode_session_file(&f, workspace) {
            sessions.push(pair);
        }
    }
    Ok(sessions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn opencode_fixture_parts_tool() {
        let dir = TempDir::new().unwrap();
        let ws = dir.path().join("myws");
        std::fs::create_dir_all(&ws).unwrap();
        let ws_canon = std::fs::canonicalize(&ws).unwrap();

        let session_path = dir.path().join("session.json");
        let body = format!(
            r#"{{
            "id": "oc-1",
            "directory": "{}",
            "model": "anthropic/claude-sonnet",
            "messages": [
                {{
                    "role": "assistant",
                    "parts": [
                        {{"type": "tool-call", "toolName": "bash", "toolCallId": "c1"}}
                    ]
                }}
            ]
        }}"#,
            ws_canon.to_string_lossy().replace('\\', "\\\\")
        );
        std::fs::write(&session_path, body).unwrap();

        let pair = parse_opencode_session_file(&session_path, &ws_canon)
            .unwrap()
            .expect("session");
        assert_eq!(pair.0.agent, "opencode");
        assert_eq!(pair.1[0].kind, EventKind::ToolCall);
        assert_eq!(pair.1[0].tool.as_deref(), Some("bash"));
    }
}
