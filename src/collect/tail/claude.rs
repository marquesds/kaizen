//! Parse Claude Code transcript `.jsonl` files into Events.
//! Pure parser — no notify dependency, no IO beyond file reads.

use crate::core::event::{Event, EventKind, EventSource, SessionRecord, SessionStatus};
use anyhow::{Context, Result};
use serde_json::Value;
use std::path::Path;

/// Parse one `.jsonl` line. Returns `Some(Event)` for action-bearing lines.
/// Extracts `tokens.inputTokens` / `tokens.outputTokens` when present.
pub fn parse_claude_line(
    session_id: &str,
    seq: u64,
    base_ts: u64,
    line: &str,
) -> Result<Option<Event>> {
    let v: Value = serde_json::from_str(line.trim()).context("claude transcript: invalid JSON")?;
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

    let tokens_in = obj
        .get("tokens")
        .and_then(|t| t.get("inputTokens"))
        .and_then(|v| v.as_u64())
        .map(|v| v as u32);

    let tokens_out = obj
        .get("tokens")
        .and_then(|t| t.get("outputTokens"))
        .and_then(|v| v.as_u64())
        .map(|v| v as u32);

    let ts_ms = base_ts + seq * 100;

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
                    kind: EventKind::ToolCall,
                    source: EventSource::Tail,
                    tool: Some(tool_name),
                    tokens_in,
                    tokens_out,
                    cost_usd_e6: None,
                    payload: block.clone(),
                }));
            }
            "tool_result" => {
                return Ok(Some(Event {
                    session_id: session_id.to_string(),
                    seq,
                    ts_ms,
                    kind: EventKind::ToolResult,
                    source: EventSource::Tail,
                    tool: None,
                    tokens_in,
                    tokens_out,
                    cost_usd_e6: None,
                    payload: block.clone(),
                }));
            }
            _ => {}
        }
    }
    Ok(None)
}

/// Walk all `.jsonl` files under `dir`; return inferred `SessionRecord` + events.
/// Agent = "claude". Session id = dir name.
pub fn scan_claude_session_dir(dir: &Path) -> Result<(SessionRecord, Vec<Event>)> {
    let session_id = dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();

    let workspace = dir
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let record = SessionRecord {
        id: session_id.clone(),
        agent: "claude".to_string(),
        model: None,
        workspace,
        started_at_ms: crate::collect::tail::dir_mtime_ms(dir),
        ended_at_ms: None,
        status: SessionStatus::Done,
        trace_path: dir.to_string_lossy().to_string(),
    };

    let mut events = Vec::new();
    let mut seq: u64 = 0;

    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .with_context(|| format!("read dir: {}", dir.display()))?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "jsonl").unwrap_or(false))
        .collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let content = std::fs::read_to_string(entry.path())?;
        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }
            if let Some(ev) = parse_claude_line(&session_id, seq, 0, line)? {
                events.push(ev);
            }
            seq += 1;
        }
    }

    Ok((record, events))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOOL_USE_LINE: &str = r#"{"role":"assistant","message":{"content":[{"type":"tool_use","id":"toolu_01","name":"read_file","input":{"path":"src/main.rs"}}]}}"#;
    const TOOL_RESULT_LINE: &str = r#"{"role":"user","message":{"content":[{"type":"tool_result","tool_use_id":"toolu_01","content":[{"type":"text","text":"fn main() {}"}]}]}}"#;
    const TEXT_ONLY_LINE: &str =
        r#"{"role":"assistant","message":{"content":[{"type":"text","text":"hello"}]}}"#;
    const WITH_TOKENS_LINE: &str = r#"{"role":"assistant","message":{"content":[{"type":"tool_use","id":"toolu_02","name":"shell","input":{"command":"cargo test"}}]},"tokens":{"inputTokens":1200,"outputTokens":800}}"#;

    #[test]
    fn parse_tool_use() {
        let ev = parse_claude_line("s1", 0, 0, TOOL_USE_LINE)
            .unwrap()
            .unwrap();
        assert_eq!(ev.kind, EventKind::ToolCall);
        assert_eq!(ev.tool.as_deref(), Some("read_file"));
        assert_eq!(ev.session_id, "s1");
    }

    #[test]
    fn parse_tool_result() {
        let ev = parse_claude_line("s1", 1, 0, TOOL_RESULT_LINE)
            .unwrap()
            .unwrap();
        assert_eq!(ev.kind, EventKind::ToolResult);
        assert_eq!(ev.seq, 1);
    }

    #[test]
    fn text_only_returns_none() {
        let result = parse_claude_line("s1", 2, 0, TEXT_ONLY_LINE).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn parse_with_tokens() {
        let ev = parse_claude_line("s1", 0, 0, WITH_TOKENS_LINE)
            .unwrap()
            .unwrap();
        assert_eq!(ev.tokens_in, Some(1200));
        assert_eq!(ev.tokens_out, Some(800));
        assert_eq!(ev.cost_usd_e6, None);
    }

    #[test]
    fn scan_fixture_dir() {
        let fixture_dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/claude");
        let (record, events) = scan_claude_session_dir(&fixture_dir).unwrap();
        assert_eq!(record.agent, "claude");
        assert!(!events.is_empty(), "expected events from fixture files");
    }
}
