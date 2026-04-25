// SPDX-License-Identifier: AGPL-3.0-or-later
//! Ingest OpenClaw sessions from `~/.openclaw/agents/*/sessions/*.jsonl`.
//! State dir: `OPENCLAW_STATE_DIR` → `OPENCLAW_HOME` → `~/.openclaw`.

use crate::collect::model_from_json;
use crate::core::event::{Event, EventKind, EventSource, SessionRecord, SessionStatus};
use anyhow::Result;
use serde_json::Value;
use std::path::{Path, PathBuf};

const AGENT: &str = "openclaw";

fn state_dir() -> PathBuf {
    if let Ok(p) = std::env::var("OPENCLAW_STATE_DIR") {
        return PathBuf::from(p);
    }
    if let Ok(h) = std::env::var("OPENCLAW_HOME") {
        return PathBuf::from(h);
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".openclaw");
    }
    PathBuf::from(".openclaw")
}

fn canonical(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

fn paths_equal(a: &Path, b: &Path) -> bool {
    canonical(a) == canonical(b)
}

/// Probe JSON object for cwd-bearing keys and compare to workspace.
fn cwd_key_matches(obj: &serde_json::Map<String, Value>, workspace: &Path) -> Option<bool> {
    for key in ["cwd", "directory", "projectPath", "root", "workspacePath"] {
        if let Some(s) = obj.get(key).and_then(|v| v.as_str()) {
            return Some(paths_equal(Path::new(s), workspace));
        }
    }
    None
}

/// Check whether a JSONL line's tool-call input contains a matching workspace path.
fn line_workspace_match(line: &str, workspace: &Path) -> Option<bool> {
    let v: Value = serde_json::from_str(line.trim()).ok()?;
    let obj = v.as_object()?;
    // Anthropic: message.content[].type=tool_use input
    if let Some(content) = obj
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_array())
    {
        for block in content {
            if block.get("type").and_then(|t| t.as_str()) == Some("tool_use")
                && let Some(input) = block.get("input").and_then(|i| i.as_object())
                && let Some(m) = cwd_key_matches(input, workspace)
            {
                return Some(m);
            }
        }
    }
    // OpenAI: tool_calls[].function.arguments (JSON string)
    if let Some(calls) = obj.get("tool_calls").and_then(|c| c.as_array()) {
        for call in calls {
            if let Some(args_str) = call
                .get("function")
                .and_then(|f| f.get("arguments"))
                .and_then(|a| a.as_str())
                && let Ok(args) = serde_json::from_str::<Value>(args_str)
                && let Some(args_obj) = args.as_object()
                && let Some(m) = cwd_key_matches(args_obj, workspace)
            {
                return Some(m);
            }
        }
    }
    // Flat object (e.g. session-level field)
    cwd_key_matches(obj, workspace)
}

/// Returns true only when file has at least one tool-call line with matching workspace.
fn file_passes_workspace_filter(path: &Path, workspace: &Path) -> bool {
    let Ok(content) = std::fs::read_to_string(path) else {
        return false;
    };
    let mut any_cwd = false;
    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        match line_workspace_match(line, workspace) {
            Some(true) => return true,
            Some(false) => any_cwd = true,
            None => {}
        }
    }
    !any_cwd
}

fn ts_from_obj(obj: &serde_json::Map<String, Value>) -> Option<u64> {
    for key in ["timestamp_ms", "ts_ms", "created_at"] {
        if let Some(v) = obj.get(key).and_then(|v| v.as_u64()) {
            return Some(if v < 1_000_000_000_000 { v * 1000 } else { v });
        }
    }
    None
}

fn tokens_in(obj: &serde_json::Map<String, Value>) -> Option<u32> {
    obj.get("usage")
        .and_then(|u| u.get("input_tokens").or_else(|| u.get("prompt_tokens")))
        .and_then(|v| v.as_u64())
        .map(|v| v as u32)
}

fn tokens_out(obj: &serde_json::Map<String, Value>) -> Option<u32> {
    obj.get("usage")
        .and_then(|u| {
            u.get("output_tokens")
                .or_else(|| u.get("completion_tokens"))
        })
        .and_then(|v| v.as_u64())
        .map(|v| v as u32)
}

/// Parse one JSONL line into an Event when it carries tool-call or tool-result content.
pub fn parse_openclaw_line(
    session_id: &str,
    seq: u64,
    base_ts: u64,
    line: &str,
) -> Result<Option<Event>> {
    let v: Value = serde_json::from_str(line.trim())?;
    let Some(obj) = v.as_object() else {
        return Ok(None);
    };
    let ts_ms = ts_from_obj(obj).unwrap_or(base_ts + seq * 100);
    let tin = tokens_in(obj);
    let tout = tokens_out(obj);

    // Anthropic content-block format
    if let Some(content) = obj
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_array())
    {
        for block in content {
            let kind = match block.get("type").and_then(|t| t.as_str()) {
                Some("tool_use") => EventKind::ToolCall,
                Some("tool_result") => EventKind::ToolResult,
                _ => continue,
            };
            let tool = block
                .get("name")
                .and_then(|n| n.as_str())
                .map(ToOwned::to_owned);
            let tcid = block
                .get("id")
                .or_else(|| block.get("tool_use_id"))
                .and_then(|v| v.as_str())
                .map(ToOwned::to_owned);
            return Ok(Some(Event {
                session_id: session_id.to_string(),
                seq,
                ts_ms,
                ts_exact: ts_from_obj(obj).is_some(),
                kind,
                source: EventSource::Tail,
                tool,
                tool_call_id: tcid,
                tokens_in: tin,
                tokens_out: tout,
                reasoning_tokens: None,
                cost_usd_e6: None,
                payload: block.clone(),
            }));
        }
    }

    // OpenAI tool_calls format
    if let Some(calls) = obj.get("tool_calls").and_then(|c| c.as_array())
        && let Some(call) = calls.first()
    {
        let tool = call
            .get("function")
            .and_then(|f| f.get("name"))
            .and_then(|n| n.as_str())
            .map(ToOwned::to_owned);
        let tcid = call
            .get("id")
            .and_then(|v| v.as_str())
            .map(ToOwned::to_owned);
        return Ok(Some(Event {
            session_id: session_id.to_string(),
            seq,
            ts_ms,
            ts_exact: ts_from_obj(obj).is_some(),
            kind: EventKind::ToolCall,
            source: EventSource::Tail,
            tool,
            tool_call_id: tcid,
            tokens_in: tin,
            tokens_out: tout,
            reasoning_tokens: None,
            cost_usd_e6: None,
            payload: call.clone(),
        }));
    }

    Ok(None)
}

/// Metadata extracted from `sessions.json` for a single session entry.
struct SessionMeta {
    model: Option<String>,
    channel: Option<String>,
    started_at_ms: Option<u64>,
}

fn read_session_meta(sessions_json: &Path, sid: &str) -> SessionMeta {
    let Ok(raw) = std::fs::read_to_string(sessions_json) else {
        return SessionMeta {
            model: None,
            channel: None,
            started_at_ms: None,
        };
    };
    let Ok(arr) = serde_json::from_str::<Value>(&raw) else {
        return SessionMeta {
            model: None,
            channel: None,
            started_at_ms: None,
        };
    };
    let Some(entries) = arr.as_array() else {
        return SessionMeta {
            model: None,
            channel: None,
            started_at_ms: None,
        };
    };
    for entry in entries {
        let entry_id = entry.get("id").and_then(|v| v.as_str()).unwrap_or("");
        if entry_id != sid {
            continue;
        }
        let model = entry
            .get("model")
            .and_then(|v| v.as_str())
            .map(ToOwned::to_owned);
        let channel = entry
            .get("channel")
            .and_then(|v| v.as_str())
            .map(ToOwned::to_owned);
        let started_at_ms = entry.get("started_at").and_then(|v| v.as_u64());
        return SessionMeta {
            model,
            channel,
            started_at_ms,
        };
    }
    SessionMeta {
        model: None,
        channel: None,
        started_at_ms: None,
    }
}

/// Ingest one `*.jsonl` session file; returns `None` when workspace filter rejects.
fn scan_session_file(
    path: &Path,
    workspace: &Path,
    agent_id: &str,
    sessions_json: &Path,
) -> Result<Option<(SessionRecord, Vec<Event>)>> {
    if !file_passes_workspace_filter(path, workspace) {
        return Ok(None);
    }
    let sid = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();
    let meta = read_session_meta(sessions_json, &sid);
    let base_ts = crate::collect::tail::dir_mtime_ms(path.parent().unwrap_or(path));
    let started_at_ms = meta.started_at_ms.unwrap_or(base_ts);

    let content = std::fs::read_to_string(path)?;
    let mut events = Vec::new();
    let mut seq: u64 = 0;
    let mut model = meta.model.clone();

    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if model.is_none() {
            model = model_from_json::from_line(line);
        }
        if let Some(mut ev) = parse_openclaw_line(&sid, seq, started_at_ms, line)? {
            if let Some(ch) = &meta.channel {
                ev.payload.as_object_mut().map(|o| {
                    o.entry("meta")
                        .or_insert_with(|| serde_json::json!({}))
                        .as_object_mut()
                        .map(|m| m.insert("channel".into(), serde_json::json!(ch)))
                });
            }
            events.push(ev);
        }
        seq += 1;
    }

    let trace_path = format!("{}/{}", agent_id, sid);
    let record = SessionRecord {
        id: sid,
        agent: AGENT.to_string(),
        model: model.or_else(|| Some(AGENT.to_string())),
        workspace: workspace.to_string_lossy().to_string(),
        started_at_ms,
        ended_at_ms: None,
        status: SessionStatus::Done,
        trace_path,
        start_commit: None,
        end_commit: None,
        branch: None,
        dirty_start: None,
        dirty_end: None,
        repo_binding_source: None,
    };
    Ok(Some((record, events)))
}

/// Scan `state_root/agents/*/sessions/*.jsonl` for sessions bound to `workspace`.
pub fn scan_openclaw_at(
    state_root: &Path,
    workspace: &Path,
) -> Result<Vec<(SessionRecord, Vec<Event>)>> {
    let agents_dir = state_root.join("agents");
    if !agents_dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut sessions = Vec::new();
    let agents = std::fs::read_dir(&agents_dir)?.filter_map(|e| e.ok());
    for agent_entry in agents {
        let sessions_dir = agent_entry.path().join("sessions");
        if !sessions_dir.is_dir() {
            continue;
        }
        let agent_id = agent_entry.file_name().to_string_lossy().to_string();
        let sessions_json = sessions_dir.join("sessions.json");
        let files: Vec<_> = std::fs::read_dir(&sessions_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map(|x| x == "jsonl").unwrap_or(false))
            .collect();
        for file_entry in files {
            if let Ok(Some(pair)) =
                scan_session_file(&file_entry.path(), workspace, &agent_id, &sessions_json)
            {
                sessions.push(pair);
            }
        }
    }
    Ok(sessions)
}

/// Scan the default OpenClaw state dir for sessions bound to `workspace`.
pub fn scan_openclaw_workspace(workspace: &Path) -> Result<Vec<(SessionRecord, Vec<Event>)>> {
    scan_openclaw_at(&state_dir(), workspace)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_anthropic_tool_use() {
        let line = r#"{"message":{"content":[{"type":"tool_use","id":"c1","name":"bash","input":{"command":"ls"}}]},"usage":{"input_tokens":100,"output_tokens":50}}"#;
        let ev = parse_openclaw_line("s1", 0, 0, line).unwrap().unwrap();
        assert_eq!(ev.kind, EventKind::ToolCall);
        assert_eq!(ev.tool.as_deref(), Some("bash"));
        assert_eq!(ev.tokens_in, Some(100));
        assert_eq!(ev.tokens_out, Some(50));
    }

    #[test]
    fn parse_openai_tool_calls() {
        let line = r#"{"tool_calls":[{"id":"call_1","function":{"name":"read_file","arguments":"{\"path\":\"src/main.rs\"}"}}],"usage":{"prompt_tokens":200,"completion_tokens":80}}"#;
        let ev = parse_openclaw_line("s1", 1, 0, line).unwrap().unwrap();
        assert_eq!(ev.kind, EventKind::ToolCall);
        assert_eq!(ev.tool.as_deref(), Some("read_file"));
        assert_eq!(ev.tokens_in, Some(200));
    }

    #[test]
    fn workspace_filter_accepts_matching_cwd() {
        let ws = Path::new("/tmp/my-project");
        let line = r#"{"message":{"content":[{"type":"tool_use","id":"c1","name":"bash","input":{"cwd":"/tmp/my-project","command":"ls"}}]}}"#;
        assert_eq!(line_workspace_match(line, ws), Some(true));
    }

    #[test]
    fn workspace_filter_rejects_wrong_cwd() {
        let ws = Path::new("/tmp/my-project");
        let line = r#"{"message":{"content":[{"type":"tool_use","id":"c1","name":"bash","input":{"cwd":"/tmp/other","command":"ls"}}]}}"#;
        assert_eq!(line_workspace_match(line, ws), Some(false));
    }

    #[test]
    fn workspace_filter_returns_none_without_cwd() {
        let ws = Path::new("/tmp/my-project");
        let line = r#"{"role":"assistant","content":"hello"}"#;
        assert_eq!(line_workspace_match(line, ws), None);
    }
}
