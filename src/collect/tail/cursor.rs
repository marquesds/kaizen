// SPDX-License-Identifier: AGPL-3.0-or-later
//! Parse Cursor agent-transcript `.jsonl` files into Events.
//! Pure parser — no notify dependency, no IO beyond file reads.

use crate::collect::model_from_json;
use crate::core::event::{Event, EventKind, EventSource, SessionRecord, SessionStatus};
use anyhow::{Context, Result};
use serde_json::Value;
use std::path::Path;

/// Parse one `.jsonl` line. Returns `Some(Event)` for action-bearing lines;
/// `None` for text-only or non-action lines.
pub fn parse_cursor_line(
    session_id: &str,
    seq: u64,
    base_ts: u64,
    line: &str,
) -> Result<Option<Event>> {
    let v: Value = serde_json::from_str(line.trim()).context("cursor transcript: invalid JSON")?;
    let obj = match v.as_object() {
        Some(o) => o,
        None => return Ok(None),
    };

    let content = obj
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_array());

    let content = match content {
        Some(c) => c,
        None => return Ok(None),
    };

    let ts_ms = line_ts_ms(obj).unwrap_or(base_ts + seq * 100);
    let ts_exact = line_ts_ms(obj).is_some();
    let reasoning_tokens = reasoning_tokens(obj);

    for block in content {
        let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
        match block_type {
            "tool_use" => {
                let tool_name = block
                    .get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("")
                    .to_string();
                return Ok(Some(Event {
                    session_id: session_id.to_string(),
                    seq,
                    ts_ms,
                    ts_exact,
                    kind: EventKind::ToolCall,
                    source: EventSource::Tail,
                    tool: Some(tool_name),
                    tool_call_id: block
                        .get("id")
                        .and_then(|v| v.as_str())
                        .map(ToOwned::to_owned),
                    tokens_in: None,
                    tokens_out: None,
                    reasoning_tokens,
                    cost_usd_e6: None,
                    payload: block.clone(),
                }));
            }
            "tool_result" => {
                return Ok(Some(Event {
                    session_id: session_id.to_string(),
                    seq,
                    ts_ms,
                    ts_exact,
                    kind: EventKind::ToolResult,
                    source: EventSource::Tail,
                    tool: None,
                    tool_call_id: block
                        .get("tool_use_id")
                        .and_then(|v| v.as_str())
                        .map(ToOwned::to_owned),
                    tokens_in: None,
                    tokens_out: None,
                    reasoning_tokens,
                    cost_usd_e6: None,
                    payload: block.clone(),
                }));
            }
            _ => {}
        }
    }
    Ok(None)
}

fn line_ts_ms(obj: &serde_json::Map<String, Value>) -> Option<u64> {
    if let Some(t) = ["timestamp_ms", "ts_ms", "created_at_ms"]
        .iter()
        .find_map(|k| obj.get(*k).and_then(|v| v.as_u64()))
    {
        return Some(t);
    }
    // Cursor often omits per-line times; `timestamp` may be seconds (≈1e9) or ms (≈1.7e12).
    if let Some(t) = obj.get("timestamp").and_then(|v| v.as_u64()) {
        return Some(if t < 1_000_000_000_000 {
            t.saturating_mul(1000)
        } else {
            t
        });
    }
    None
}

fn reasoning_tokens(obj: &serde_json::Map<String, Value>) -> Option<u32> {
    obj.get("usage")
        .and_then(|u| u.get("reasoning_tokens"))
        .and_then(|v| v.as_u64())
        .map(|v| v as u32)
        .or_else(|| {
            obj.get("tokens")
                .and_then(|u| u.get("reasoningTokens"))
                .and_then(|v| v.as_u64())
                .map(|v| v as u32)
        })
}

fn file_mtime_ms(path: &Path) -> u64 {
    path.metadata()
        .ok()
        .and_then(|m| m.modified().ok())
        .map(|t| {
            t.duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64
        })
        .unwrap_or(0)
}

/// Read every `*.jsonl` directly under `dir` (sorted by name) and parse into events.
/// First `model` (or supported nested field) found in any line is returned for the session.
fn scan_jsonl_in_dir(dir: &Path, session_id: &str) -> Result<(Vec<Event>, Option<String>)> {
    // Transcript lines often omit `timestamp_ms`; align synthetic times with the session
    // dir mtime (same as `SessionRecord.started_at_ms`) so retro windows and queries match.
    let base_ts = super::dir_mtime_ms(dir);
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .with_context(|| format!("read dir: {}", dir.display()))?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "jsonl").unwrap_or(false))
        .collect();
    entries.sort_by_key(|e| e.file_name());

    let mut events = Vec::new();
    let mut seq: u64 = 0;
    let mut model: Option<String> = None;
    for entry in entries {
        let content = std::fs::read_to_string(entry.path())?;
        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }
            if model.is_none() {
                model = model_from_json::from_line(line);
            }
            if let Some(ev) = parse_cursor_line(session_id, seq, base_ts, line)? {
                events.push(ev);
                seq += 1;
            } else {
                seq += 1;
            }
        }
    }
    Ok((events, model))
}

/// Parse a single transcript `.jsonl` file into events.
fn scan_jsonl_file(path: &Path, session_id: &str) -> Result<(Vec<Event>, Option<String>)> {
    let base_ts = file_mtime_ms(path);
    let content =
        std::fs::read_to_string(path).with_context(|| format!("read file: {}", path.display()))?;
    let mut events = Vec::new();
    let mut seq: u64 = 0;
    let mut model: Option<String> = None;
    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if model.is_none() {
            model = model_from_json::from_line(line);
        }
        if let Some(ev) = parse_cursor_line(session_id, seq, base_ts, line)? {
            events.push(ev);
            seq += 1;
        } else {
            seq += 1;
        }
    }
    Ok((events, model))
}

fn cursor_workspace_for_session_dir(dir: &Path) -> String {
    dir.parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default()
}

/// Main session plus one session per `subagents/*.jsonl` (Cursor subagent transcripts).
pub fn scan_session_dir_all(dir: &Path) -> Result<Vec<(SessionRecord, Vec<Event>)>> {
    let session_id = dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();

    let workspace = cursor_workspace_for_session_dir(dir);

    let (main_events, main_model) = scan_jsonl_in_dir(dir, &session_id)?;

    let main_record = SessionRecord {
        id: session_id.clone(),
        agent: "cursor".to_string(),
        model: main_model,
        workspace: workspace.clone(),
        started_at_ms: crate::collect::tail::dir_mtime_ms(dir),
        ended_at_ms: None,
        status: SessionStatus::Done,
        trace_path: dir.to_string_lossy().to_string(),
        start_commit: None,
        end_commit: None,
        branch: None,
        dirty_start: None,
        dirty_end: None,
        repo_binding_source: None,
    };

    let mut out = vec![(main_record, main_events)];

    let subagents = dir.join("subagents");
    if subagents.is_dir() {
        let mut subs: Vec<_> = std::fs::read_dir(&subagents)
            .with_context(|| format!("read dir: {}", subagents.display()))?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map(|x| x == "jsonl").unwrap_or(false))
            .collect();
        subs.sort_by_key(|e| e.file_name());

        for entry in subs {
            let path = entry.path();
            let sub_id = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            if sub_id.is_empty() {
                continue;
            }
            let (events, sub_model) = scan_jsonl_file(&path, &sub_id)?;
            let record = SessionRecord {
                id: sub_id.clone(),
                agent: "cursor".to_string(),
                model: sub_model,
                workspace: workspace.clone(),
                started_at_ms: file_mtime_ms(&path),
                ended_at_ms: None,
                status: SessionStatus::Done,
                trace_path: path.to_string_lossy().to_string(),
                start_commit: None,
                end_commit: None,
                branch: None,
                dirty_start: None,
                dirty_end: None,
                repo_binding_source: None,
            };
            out.push((record, events));
        }
    }

    Ok(out)
}

/// Walk all `.jsonl` files directly under `dir`; return inferred `SessionRecord` + events.
///
/// Session id = dir name (last path component).
/// Agent = "cursor". workspace = parent of parent (assuming `.../agent-transcripts/<id>`).
/// status = Done (static dir assumed completed).
///
/// Does not include `subagents/*.jsonl`; use [`scan_session_dir_all`] for full ingestion.
pub fn scan_session_dir(dir: &Path) -> Result<(SessionRecord, Vec<Event>)> {
    let session_id = dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();
    let workspace = cursor_workspace_for_session_dir(dir);
    let (events, model) = scan_jsonl_in_dir(dir, &session_id)?;
    let record = SessionRecord {
        id: session_id.clone(),
        agent: "cursor".to_string(),
        model,
        workspace,
        started_at_ms: crate::collect::tail::dir_mtime_ms(dir),
        ended_at_ms: None,
        status: SessionStatus::Done,
        trace_path: dir.to_string_lossy().to_string(),
        start_commit: None,
        end_commit: None,
        branch: None,
        dirty_start: None,
        dirty_end: None,
        repo_binding_source: None,
    };
    Ok((record, events))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOOL_USE_LINE: &str = r#"{"role":"assistant","message":{"content":[{"type":"tool_use","id":"toolu_01","name":"read_file","input":{"path":"src/main.rs"}}]}}"#;
    const TOOL_RESULT_LINE: &str = r#"{"role":"user","message":{"content":[{"type":"tool_result","tool_use_id":"toolu_01","content":[{"type":"text","text":"fn main() {}"}]}]}}"#;
    const TEXT_ONLY_LINE: &str =
        r#"{"role":"assistant","message":{"content":[{"type":"text","text":"hello"}]}}"#;

    #[test]
    fn parse_tool_use() {
        let ev = parse_cursor_line("s1", 0, 0, TOOL_USE_LINE)
            .unwrap()
            .unwrap();
        assert_eq!(ev.kind, EventKind::ToolCall);
        assert_eq!(ev.tool.as_deref(), Some("read_file"));
        assert_eq!(ev.tool_call_id.as_deref(), Some("toolu_01"));
        assert_eq!(ev.session_id, "s1");
    }

    #[test]
    fn parse_tool_result() {
        let ev = parse_cursor_line("s1", 1, 0, TOOL_RESULT_LINE)
            .unwrap()
            .unwrap();
        assert_eq!(ev.kind, EventKind::ToolResult);
        assert_eq!(ev.seq, 1);
        assert_eq!(ev.tool_call_id.as_deref(), Some("toolu_01"));
    }

    #[test]
    fn text_only_returns_none() {
        let result = parse_cursor_line("s1", 2, 0, TEXT_ONLY_LINE).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn ts_ms_synthesized() {
        let ev = parse_cursor_line("s1", 3, 1000, TOOL_USE_LINE)
            .unwrap()
            .unwrap();
        assert_eq!(ev.ts_ms, 1000 + 3 * 100);
    }

    #[test]
    fn scan_fixture_dir() {
        let fixture_dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cursor");
        let (record, events) = scan_session_dir(&fixture_dir).unwrap();
        assert_eq!(record.agent, "cursor");
        assert_eq!(record.model.as_deref(), Some("Test Fixture Model"));
        assert_eq!(record.status, SessionStatus::Done);
        assert!(!events.is_empty(), "expected events from fixture files");
        assert!(events.iter().any(|e| e.kind == EventKind::ToolCall));
        assert!(events.iter().any(|e| e.kind == EventKind::ToolResult));
    }

    #[test]
    fn scan_session_dir_all_includes_subagents() {
        let fixture_dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cursor");
        let sessions = scan_session_dir_all(&fixture_dir).unwrap();
        assert!(
            sessions.len() >= 2,
            "expected main session + subagent fixture"
        );
        let sub_id = "a1b2c3d4-e5f6-7890-abcd-ef1234567890";
        let sub = sessions
            .iter()
            .find(|(r, _)| r.id == sub_id)
            .expect("subagent session");
        assert_eq!(sub.0.agent, "cursor");
        assert!(
            sub.0.trace_path.ends_with(".jsonl"),
            "subagent trace_path should be file path"
        );
        assert!(
            sub.1.iter().any(|e| e.tool.as_deref() == Some("grep")),
            "subagent tool call"
        );
    }
}
