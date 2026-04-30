// SPDX-License-Identifier: AGPL-3.0-or-later
//! Parse Codex (OpenAI) transcript `.jsonl` files into Events.
//! Pure parser — no notify dependency, no IO beyond file reads.

use crate::collect::model_from_json;
use crate::core::cost::estimate_tail_event_cost_usd_e6;
use crate::core::event::{Event, EventKind, EventSource, SessionRecord, SessionStatus};
use anyhow::{Context, Result};
use serde_json::Value;
use std::path::Path;

/// Parse one `.jsonl` line. Returns `Some(Event)` for action-bearing lines.
/// Extracts `usage.prompt_tokens` / `usage.completion_tokens` when present.
pub fn parse_codex_line(
    session_id: &str,
    seq: u64,
    base_ts: u64,
    line: &str,
) -> Result<Option<Event>> {
    let v: Value = serde_json::from_str(line.trim()).context("codex transcript: invalid JSON")?;
    let obj = match v.as_object() {
        Some(o) => o,
        None => return Ok(None),
    };

    let tokens_in = obj
        .get("usage")
        .and_then(|u| u.get("prompt_tokens"))
        .and_then(|v| v.as_u64())
        .map(|v| v as u32);

    let tokens_out = obj
        .get("usage")
        .and_then(|u| u.get("completion_tokens"))
        .and_then(|v| v.as_u64())
        .map(|v| v as u32);

    let reasoning_tokens = obj
        .get("usage")
        .and_then(|u| u.get("completion_tokens_details"))
        .and_then(|d| d.get("reasoning_tokens"))
        .and_then(|v| v.as_u64())
        .map(|v| v as u32)
        .or_else(|| {
            obj.get("usage")
                .and_then(|u| u.get("output_tokens_details"))
                .and_then(|d| d.get("reasoning_tokens"))
                .and_then(|v| v.as_u64())
                .map(|v| v as u32)
        });

    let ts_ms = line_ts_ms(obj).unwrap_or(base_ts + seq * 100);
    let ts_exact = line_ts_ms(obj).is_some();
    let line_model = model_from_json::from_object(obj);
    let cost_usd_e6 = estimate_tail_event_cost_usd_e6(
        line_model.as_deref(),
        tokens_in,
        tokens_out,
        reasoning_tokens,
    );

    // Tool calls are in top-level `tool_calls` array
    if let Some(first) = obj
        .get("tool_calls")
        .and_then(|tc| tc.as_array())
        .and_then(|arr| arr.first())
    {
        let tool_name = first
            .get("function")
            .and_then(|f| f.get("name"))
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
            tool_call_id: first
                .get("id")
                .and_then(|v| v.as_str())
                .map(ToOwned::to_owned),
            tokens_in,
            tokens_out,
            reasoning_tokens,
            cost_usd_e6,
            stop_reason: None,
            latency_ms: None,
            ttft_ms: None,
            retry_count: None,
            context_used_tokens: None,
            context_max_tokens: None,
            cache_creation_tokens: None,
            cache_read_tokens: None,
            system_prompt_tokens: None,
            payload: first.clone(),
        }));
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
    if let Some(t) = obj.get("timestamp").and_then(|v| v.as_u64()) {
        return Some(if t < 1_000_000_000_000 {
            t.saturating_mul(1000)
        } else {
            t
        });
    }
    None
}

/// Walk all `.jsonl` files under `dir`; return inferred `SessionRecord` + events.
/// Agent = "codex". Session id = dir name.
pub fn scan_codex_session_dir(dir: &Path) -> Result<(SessionRecord, Vec<Event>)> {
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

    let mut events = Vec::new();
    let mut seq: u64 = 0;
    let mut model: Option<String> = None;

    let base_ts = crate::collect::tail::dir_mtime_ms(dir);
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
            if let Some(m) = model_from_json::from_line(line) {
                model = Some(m);
            }
            if let Some(ev) = parse_codex_line(&session_id, seq, base_ts, line)? {
                events.push(ev);
            }
            seq += 1;
        }
    }

    let record = SessionRecord {
        id: session_id.clone(),
        agent: "codex".to_string(),
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
        prompt_fingerprint: None,
        parent_session_id: None,
        agent_version: None,
        os: None,
        arch: None,
        repo_file_count: None,
        repo_total_loc: None,
    };
    Ok((record, events))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOOL_USE_LINE: &str = r#"{"role":"assistant","content":null,"tool_calls":[{"id":"call_01","type":"function","function":{"name":"shell","arguments":"{\"command\":\"ls\"}"}}]}"#;
    const WITH_USAGE_LINE: &str = r#"{"role":"assistant","content":null,"tool_calls":[{"id":"call_02","type":"function","function":{"name":"read_file","arguments":"{\"path\":\"src/main.rs\"}"}}],"usage":{"prompt_tokens":500,"completion_tokens":300,"total_tokens":800}}"#;

    #[test]
    fn parse_tool_use() {
        let ev = parse_codex_line("s1", 0, 0, TOOL_USE_LINE)
            .unwrap()
            .unwrap();
        assert_eq!(ev.kind, EventKind::ToolCall);
        assert_eq!(ev.tool.as_deref(), Some("shell"));
        assert_eq!(ev.tool_call_id.as_deref(), Some("call_01"));
        assert_eq!(ev.session_id, "s1");
    }

    #[test]
    fn parse_with_usage() {
        let ev = parse_codex_line("s1", 0, 0, WITH_USAGE_LINE)
            .unwrap()
            .unwrap();
        assert_eq!(ev.tokens_in, Some(500));
        assert_eq!(ev.tokens_out, Some(300));
        assert!(
            ev.cost_usd_e6.is_some_and(|c| c > 0),
            "expected tail cost from usage"
        );
    }

    #[test]
    fn scan_fixture_dir() {
        let fixture_dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/codex");
        let (record, events) = scan_codex_session_dir(&fixture_dir).unwrap();
        assert_eq!(record.agent, "codex");
        assert_eq!(record.model.as_deref(), Some("gpt-4o-fixture"));
        assert!(!events.is_empty(), "expected events from fixture files");
        assert!(
            events.iter().any(|e| e.cost_usd_e6.is_some_and(|c| c > 0)),
            "with_usage fixture should yield cost"
        );
    }
}
