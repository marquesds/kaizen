// SPDX-License-Identifier: AGPL-3.0-or-later
//! Ingest GitHub Copilot CLI sessions from `~/.copilot/session-state/<id>/events.jsonl`.

use crate::collect::model_from_json;
use crate::collect::tail::dir_mtime_ms;
use crate::core::event::{Event, EventKind, EventSource, SessionRecord, SessionStatus};
use anyhow::{Context, Result};
use serde_json::Value;
use std::path::{Path, PathBuf};

const AGENT: &str = "copilot-cli";

fn copilot_home() -> PathBuf {
    if let Ok(p) = std::env::var("COPILOT_HOME") {
        return PathBuf::from(p);
    }
    if let Ok(h) = std::env::var("HOME") {
        return PathBuf::from(h).join(".copilot");
    }
    PathBuf::from(".copilot")
}

fn canonical(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

fn paths_equal(a: &Path, b: &Path) -> bool {
    canonical(a) == canonical(b)
}

fn session_workspace_path(session_dir: &Path) -> Option<PathBuf> {
    let wj = session_dir.join("workspace.json");
    if let Ok(text) = std::fs::read_to_string(&wj)
        && let Ok(v) = serde_json::from_str::<Value>(&text)
    {
        for key in ["workspaceFolder", "cwd", "workingDirectory", "folder"] {
            if let Some(s) = v.get(key).and_then(|x| x.as_str()) {
                let p = s.strip_prefix("file://").unwrap_or(s);
                return Some(PathBuf::from(p));
            }
        }
    }
    let meta = session_dir.join("metadata.json");
    if let Ok(text) = std::fs::read_to_string(&meta)
        && let Ok(v) = serde_json::from_str::<Value>(&text)
        && let Some(s) = v
            .get("workspaceFolder")
            .or_else(|| v.get("cwd"))
            .and_then(|x| x.as_str())
    {
        let p = s.strip_prefix("file://").unwrap_or(s);
        return Some(PathBuf::from(p));
    }
    None
}

/// Parse one line from Copilot CLI `events.jsonl`.
pub fn parse_copilot_cli_line(
    session_id: &str,
    seq: u64,
    base_ts: u64,
    line: &str,
) -> Result<Option<Event>> {
    let v: Value = serde_json::from_str(line.trim()).context("copilot cli jsonl")?;
    let obj = match v.as_object() {
        Some(o) => o,
        None => return Ok(None),
    };

    let ts_ms = obj
        .get("timestamp_ms")
        .or_else(|| obj.get("timestamp"))
        .and_then(|t| t.as_u64())
        .unwrap_or(base_ts + seq);

    if let Some(tool_calls) = obj.get("tool_calls").and_then(|t| t.as_array())
        && let Some(first) = tool_calls.first()
    {
        let tool_name = first
            .get("function")
            .and_then(|f| f.get("name"))
            .or_else(|| first.get("name"))
            .and_then(|n| n.as_str())
            .unwrap_or("")
            .to_string();
        return Ok(Some(Event {
            session_id: session_id.to_string(),
            seq,
            ts_ms,
            ts_exact: true,
            kind: EventKind::ToolCall,
            source: EventSource::Tail,
            tool: Some(tool_name),
            tool_call_id: first
                .get("id")
                .and_then(|x| x.as_str())
                .map(ToOwned::to_owned),
            tokens_in: obj
                .get("usage")
                .and_then(|u| u.get("prompt_tokens"))
                .and_then(|x| x.as_u64())
                .map(|x| x as u32),
            tokens_out: obj
                .get("usage")
                .and_then(|u| u.get("completion_tokens"))
                .and_then(|x| x.as_u64())
                .map(|x| x as u32),
            reasoning_tokens: None,
            cost_usd_e6: None,
            payload: v.clone(),
        }));
    }

    if let Some(name) = obj
        .get("tool")
        .and_then(|t| t.get("name"))
        .or_else(|| obj.get("toolName"))
        .and_then(|n| n.as_str())
    {
        return Ok(Some(Event {
            session_id: session_id.to_string(),
            seq,
            ts_ms,
            ts_exact: true,
            kind: EventKind::ToolCall,
            source: EventSource::Tail,
            tool: Some(name.to_string()),
            tool_call_id: obj
                .get("tool_call_id")
                .or_else(|| obj.get("id"))
                .and_then(|x| x.as_str())
                .map(ToOwned::to_owned),
            tokens_in: None,
            tokens_out: None,
            reasoning_tokens: None,
            cost_usd_e6: None,
            payload: v.clone(),
        }));
    }

    Ok(None)
}

/// Scan one Copilot CLI session directory if it belongs to `workspace`.
pub fn scan_copilot_cli_session_dir(
    session_dir: &Path,
    workspace: &Path,
) -> Result<Option<(SessionRecord, Vec<Event>)>> {
    let events_path = session_dir.join("events.jsonl");
    if !events_path.is_file() {
        return Ok(None);
    }

    let ws_match = if let Some(w) = session_workspace_path(session_dir) {
        paths_equal(&w, workspace)
    } else {
        false
    };
    if !ws_match {
        return Ok(None);
    }

    let session_id = session_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("copilot-cli")
        .to_string();

    let content = std::fs::read_to_string(&events_path)?;
    let mut events = Vec::new();
    let mut seq: u64 = 0;
    let mut model: Option<String> = None;
    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if model.is_none()
            && let Ok(v) = serde_json::from_str::<Value>(line)
        {
            model = model_from_json::from_value(&v);
        }
        if let Some(ev) = parse_copilot_cli_line(&session_id, seq, 0, line)? {
            events.push(ev);
        }
        seq += 1;
    }

    if events.is_empty() {
        return Ok(None);
    }

    Ok(Some((
        SessionRecord {
            id: session_id,
            agent: AGENT.to_string(),
            model,
            workspace: workspace.to_string_lossy().to_string(),
            started_at_ms: dir_mtime_ms(session_dir),
            ended_at_ms: None,
            status: SessionStatus::Done,
            trace_path: session_dir.to_string_lossy().to_string(),
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

/// All Copilot CLI sessions for this workspace.
pub fn scan_copilot_cli_workspace(workspace: &Path) -> Result<Vec<(SessionRecord, Vec<Event>)>> {
    let home = copilot_home();
    let state = home.join("session-state");
    if !state.is_dir() {
        return Ok(vec![]);
    }
    let mut out = Vec::new();
    for e in std::fs::read_dir(&state)? {
        let e = e?;
        let p = e.path();
        if !p.is_dir() {
            continue;
        }
        if let Some(pair) = scan_copilot_cli_session_dir(&p, workspace)? {
            out.push(pair);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn copilot_cli_tool_calls_line() {
        let line = r#"{"role":"assistant","tool_calls":[{"id":"call_1","type":"function","function":{"name":"run_terminal_cmd","arguments":"{}"}}],"timestamp_ms":1000}"#;
        let ev = parse_copilot_cli_line("s1", 0, 0, line).unwrap().unwrap();
        assert_eq!(ev.kind, EventKind::ToolCall);
        assert_eq!(ev.tool.as_deref(), Some("run_terminal_cmd"));
    }

    #[test]
    fn copilot_cli_session_fixture() {
        let dir = TempDir::new().unwrap();
        let ws = dir.path().join("repo");
        std::fs::create_dir_all(&ws).unwrap();
        let ws_canon = std::fs::canonicalize(&ws).unwrap();

        let sess = dir.path().join("session-state/sess-abc");
        std::fs::create_dir_all(&sess).unwrap();
        std::fs::write(
            sess.join("workspace.json"),
            format!(
                r#"{{"workspaceFolder": "{}"}}"#,
                ws_canon.to_string_lossy().replace('\\', "\\\\")
            ),
        )
        .unwrap();
        let line = r#"{"role":"assistant","tool_calls":[{"id":"c1","type":"function","function":{"name":"read_file","arguments":"{}"}}],"timestamp_ms":5000}"#;
        std::fs::write(sess.join("events.jsonl"), line).unwrap();

        let pair = scan_copilot_cli_session_dir(&sess, &ws_canon)
            .unwrap()
            .expect("pair");
        assert_eq!(pair.0.agent, "copilot-cli");
        assert!(!pair.1.is_empty());
    }
}
