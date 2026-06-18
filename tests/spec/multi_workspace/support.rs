// SPDX-License-Identifier: AGPL-3.0-or-later

use kaizen::DataSource;
use kaizen::core::event::{Event, EventKind, EventSource, SessionRecord, SessionStatus};
use kaizen::shell::cli::{sessions_list_text, summary_text};
use kaizen::shell::load::load_text;
use kaizen::shell::scope;
use kaizen::store::{SYNC_STATE_LAST_AGENT_SCAN_MS, Store};
use serde_json::json;
use std::path::Path;
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use tempfile::TempDir;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn seed_session(workspace: &Path, id: &str, tool: &str) -> anyhow::Result<()> {
    let workspace = std::fs::canonicalize(workspace)?;
    let store = Store::open(&kaizen::core::workspace::db_path(&workspace)?)?;
    let ws = workspace.to_string_lossy().to_string();
    store.upsert_session(&SessionRecord {
        id: id.into(),
        agent: "codex".into(),
        model: Some("gpt-5.4".into()),
        workspace: ws.clone(),
        started_at_ms: 1,
        ended_at_ms: Some(2),
        status: SessionStatus::Done,
        trace_path: workspace.join("trace.jsonl").to_string_lossy().to_string(),
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
    })?;
    store.append_event(&Event {
        session_id: id.into(),
        seq: 0,
        ts_ms: 1,
        ts_exact: true,
        kind: EventKind::ToolCall,
        source: EventSource::Hook,
        tool: Some(tool.into()),
        tool_call_id: Some(format!("{id}-call")),
        tokens_in: Some(10),
        tokens_out: Some(20),
        reasoning_tokens: Some(5),
        cost_usd_e6: Some(123),
        stop_reason: None,
        latency_ms: None,
        ttft_ms: None,
        retry_count: None,
        context_used_tokens: None,
        context_max_tokens: None,
        cache_creation_tokens: None,
        cache_read_tokens: None,
        system_prompt_tokens: None,
        payload: json!({"tool": tool}),
    })?;
    Ok(())
}

fn set_env(name: &str, value: impl AsRef<std::ffi::OsStr>) {
    unsafe { std::env::set_var(name, value) };
}

fn clear_env(name: &str) {
    unsafe { std::env::remove_var(name) };
}

fn recent_timestamp() -> anyhow::Result<String> {
    Ok(OffsetDateTime::now_utc().format(&Rfc3339)?)
}

fn write_codex_session(home: &Path, workspace: &Path, id: &str) -> anyhow::Result<()> {
    let dir = home.join(".codex/sessions/2026/05/15");
    std::fs::create_dir_all(&dir)?;
    let ws = workspace.to_string_lossy().to_string();
    let timestamp = recent_timestamp()?;
    let lines = vec![
        json!({"timestamp":timestamp,"type":"session_meta","payload":{"id":id,"cwd":ws}}),
        json!({"timestamp":timestamp,"type":"turn_context","payload":{"type":"turn_context","cwd":ws,"model":"gpt-4o"}}),
        json!({"timestamp":timestamp,"type":"response_item","payload":{"type":"function_call","call_id":"call_1","name":"exec_command","arguments":"{}"}}),
    ];
    std::fs::write(dir.join(format!("{id}.jsonl")), join_jsonl(lines))?;
    Ok(())
}

fn write_claude_session(home: &Path, workspace: &Path, id: &str) -> anyhow::Result<()> {
    let slug = kaizen::core::paths::claude_code_slug(workspace);
    let dir = home.join(".claude/projects").join(slug);
    std::fs::create_dir_all(&dir)?;
    let ws = workspace.to_string_lossy().to_string();
    let timestamp = recent_timestamp()?;
    let lines = vec![
        json!({"type":"assistant","timestamp":timestamp,"cwd":ws,"sessionId":id,"model":"claude-sonnet-4","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_1","name":"Bash","input":{"command":"cargo test"}}]}}),
    ];
    std::fs::write(dir.join(format!("{id}.jsonl")), join_jsonl(lines))?;
    Ok(())
}

fn write_gemini_session(workspace: &Path, id: &str) -> anyhow::Result<()> {
    let dir = workspace.join(".gemini");
    std::fs::create_dir_all(&dir)?;
    let ws = workspace.to_string_lossy().to_string();
    let timestamp = recent_timestamp()?;
    let lines = vec![
        json!({"type":"session","session_id":id,"cwd":ws,"model":"gemini-2.5-pro","timestamp":timestamp}),
        json!({"timestamp":timestamp,"message":{"content":[{"type":"tool_use","id":"g1","name":"read_file"}]}}),
    ];
    std::fs::write(dir.join(format!("{id}.jsonl")), join_jsonl(lines))?;
    Ok(())
}

fn join_jsonl(lines: Vec<serde_json::Value>) -> String {
    let mut text = lines
        .into_iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    text.push('\n');
    text
}
