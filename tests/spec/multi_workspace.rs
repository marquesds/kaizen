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

fn write_codex_session(home: &Path, workspace: &Path, id: &str) -> anyhow::Result<()> {
    let dir = home.join(".codex/sessions/2026/05/15");
    std::fs::create_dir_all(&dir)?;
    let ws = workspace.to_string_lossy().to_string();
    let lines = vec![
        json!({"timestamp":"2026-05-15T20:47:40Z","type":"session_meta","payload":{"id":id,"cwd":ws}}),
        json!({"timestamp":"2026-05-15T20:47:41Z","type":"turn_context","payload":{"type":"turn_context","cwd":ws,"model":"gpt-4o"}}),
        json!({"timestamp":"2026-05-15T20:47:42Z","type":"response_item","payload":{"type":"function_call","call_id":"call_1","name":"exec_command","arguments":"{}"}}),
    ];
    std::fs::write(dir.join(format!("{id}.jsonl")), join_jsonl(lines))?;
    Ok(())
}

fn write_claude_session(home: &Path, workspace: &Path, id: &str) -> anyhow::Result<()> {
    let slug = kaizen::core::paths::claude_code_slug(workspace);
    let dir = home.join(".claude/projects").join(slug);
    std::fs::create_dir_all(&dir)?;
    let ws = workspace.to_string_lossy().to_string();
    let lines = vec![
        json!({"type":"assistant","timestamp":"2026-05-15T20:47:40Z","cwd":ws,"sessionId":id,"model":"claude-sonnet-4","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_1","name":"Bash","input":{"command":"cargo test"}}]}}),
    ];
    std::fs::write(dir.join(format!("{id}.jsonl")), join_jsonl(lines))?;
    Ok(())
}

fn write_gemini_session(workspace: &Path, id: &str) -> anyhow::Result<()> {
    let dir = workspace.join(".gemini");
    std::fs::create_dir_all(&dir)?;
    let ws = workspace.to_string_lossy().to_string();
    let lines = vec![
        json!({"type":"session","session_id":id,"cwd":ws,"model":"gemini-2.5-pro","timestamp":"2026-05-15T20:47:40Z"}),
        json!({"timestamp":"2026-05-15T20:47:41Z","message":{"content":[{"type":"tool_use","id":"g1","name":"read_file"}]}}),
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

#[test]
fn summary_aggregates_registered_workspaces() -> anyhow::Result<()> {
    let _guard = env_lock().lock().unwrap_or_else(|e| e.into_inner());
    let home = TempDir::new()?;
    let ws1 = home.path().join("repo-a");
    let ws2 = home.path().join("repo-b");
    std::fs::create_dir_all(&ws1)?;
    std::fs::create_dir_all(&ws2)?;
    let ws1 = std::fs::canonicalize(ws1)?;
    let ws2 = std::fs::canonicalize(ws2)?;
    set_env("HOME", home.path());
    set_env("KAIZEN_HOME", home.path().join(".kaizen"));
    kaizen::core::workspace::resolve(Some(&ws1))?;
    kaizen::core::workspace::resolve(Some(&ws2))?;
    seed_session(&ws1, "s1", "read_file")?;
    seed_session(&ws2, "s2", "shell")?;

    let text = summary_text(Some(&ws1), true, false, true, DataSource::Local)?;
    let json: serde_json::Value = serde_json::from_str(&text)?;
    assert_eq!(json["session_count"], 2);
    assert_eq!(json["workspaces"].as_array().map(|v| v.len()), Some(2));

    clear_env("KAIZEN_HOME");
    clear_env("HOME");
    Ok(())
}

#[test]
fn all_workspaces_includes_init_only_root_without_local_kaizen_db() -> anyhow::Result<()> {
    let _guard = env_lock().lock().unwrap_or_else(|e| e.into_inner());
    let home = TempDir::new()?;
    let ws1 = home.path().join("repo-a");
    let ws2 = home.path().join("repo-b");
    std::fs::create_dir_all(&ws1)?;
    std::fs::create_dir_all(&ws2)?;
    let ws1 = std::fs::canonicalize(&ws1)?;
    let ws2 = std::fs::canonicalize(&ws2)?;
    set_env("HOME", home.path());
    set_env("KAIZEN_HOME", home.path().join(".kaizen"));
    kaizen::core::workspace::resolve(Some(&ws1))?;
    kaizen::core::machine_registry::record_init(&ws2)?;
    assert!(
        !kaizen::core::workspace::db_path(&ws2).is_ok_and(|p| p.exists()),
        "test assumes second repo has no kaizen.db yet"
    );
    let roots = scope::resolve(Some(&ws1), true)?;
    assert_eq!(roots.len(), 2);
    assert!(roots.contains(&ws1));
    assert!(roots.contains(&ws2));
    clear_env("KAIZEN_HOME");
    clear_env("HOME");
    Ok(())
}

#[test]
fn sessions_list_stays_repo_scoped_without_machine_flag() -> anyhow::Result<()> {
    let _guard = env_lock().lock().unwrap_or_else(|e| e.into_inner());
    let home = TempDir::new()?;
    let ws1 = home.path().join("repo-a");
    let ws2 = home.path().join("repo-b");
    std::fs::create_dir_all(&ws1)?;
    std::fs::create_dir_all(&ws2)?;
    let ws1 = std::fs::canonicalize(ws1)?;
    let ws2 = std::fs::canonicalize(ws2)?;
    set_env("HOME", home.path());
    set_env("KAIZEN_HOME", home.path().join(".kaizen"));
    seed_session(&ws1, "s1", "read_file")?;
    seed_session(&ws2, "s2", "shell")?;

    let text = sessions_list_text(Some(&ws1), true, false, false, None)?;
    let json: serde_json::Value = serde_json::from_str(&text)?;
    assert_eq!(json["count"], 1);
    assert_eq!(json["sessions"][0]["id"], "s1");

    clear_env("KAIZEN_HOME");
    clear_env("HOME");
    Ok(())
}

#[test]
fn default_reads_skip_global_scan_until_refresh() -> anyhow::Result<()> {
    let _guard = env_lock().lock().unwrap_or_else(|e| e.into_inner());
    let home = TempDir::new()?;
    let ws = home.path().join("repo");
    std::fs::create_dir_all(&ws)?;
    let ws = std::fs::canonicalize(ws)?;
    set_env("HOME", home.path());
    set_env("KAIZEN_HOME", home.path().join(".kaizen"));
    let cursor_slug = kaizen::core::paths::cursor_slug(&ws);
    let session = home
        .path()
        .join(".cursor/projects")
        .join(cursor_slug)
        .join("agent-transcripts/session-1");
    std::fs::create_dir_all(&session)?;
    std::fs::write(
        session.join("000.jsonl"),
        r#"{"message":{"content":[{"type":"tool_use","id":"toolu_1","name":"read_file","input":{"path":"src/main.rs"}}]}}"#,
    )?;

    let cold = sessions_list_text(Some(&ws), true, false, false, None)?;
    let cold_json: serde_json::Value = serde_json::from_str(&cold)?;
    assert_eq!(cold_json["count"], 0);
    let store = Store::open(&kaizen::core::workspace::db_path(&ws)?)?;
    assert_eq!(
        store.sync_state_get_u64(SYNC_STATE_LAST_AGENT_SCAN_MS)?,
        None
    );

    let refreshed = sessions_list_text(Some(&ws), true, true, false, None)?;
    let refreshed_json: serde_json::Value = serde_json::from_str(&refreshed)?;
    assert_eq!(refreshed_json["count"], 1);
    assert!(
        Store::open(&kaizen::core::workspace::db_path(&ws)?)?
            .sync_state_get_u64(SYNC_STATE_LAST_AGENT_SCAN_MS)?
            .is_some()
    );

    clear_env("KAIZEN_HOME");
    clear_env("HOME");
    Ok(())
}

#[test]
fn sessions_list_refresh_sees_modern_agent_logs() -> anyhow::Result<()> {
    let _guard = env_lock().lock().unwrap_or_else(|e| e.into_inner());
    let home = TempDir::new()?;
    let ws = home.path().join("repo");
    std::fs::create_dir_all(&ws)?;
    let ws = std::fs::canonicalize(ws)?;
    set_env("HOME", home.path());
    set_env("KAIZEN_HOME", home.path().join(".kaizen"));
    write_codex_session(home.path(), &ws, "codex-refresh-1")?;
    write_claude_session(home.path(), &ws, "claude-refresh-1")?;
    write_gemini_session(&ws, "gemini-refresh-1")?;

    let text = sessions_list_text(Some(&ws), true, true, false, None)?;
    let json: serde_json::Value = serde_json::from_str(&text)?;
    assert_eq!(json["count"], 3);

    clear_env("KAIZEN_HOME");
    clear_env("HOME");
    Ok(())
}

#[test]
fn sessions_list_defaults_to_100_and_limit_zero_returns_all() -> anyhow::Result<()> {
    let _guard = env_lock().lock().unwrap_or_else(|e| e.into_inner());
    let home = TempDir::new()?;
    let ws = home.path().join("repo");
    std::fs::create_dir_all(&ws)?;
    let ws = std::fs::canonicalize(ws)?;
    set_env("HOME", home.path());
    set_env("KAIZEN_HOME", home.path().join(".kaizen"));
    for i in 0..105 {
        seed_session(&ws, &format!("s{i:03}"), "read_file")?;
    }

    let capped = sessions_list_text(Some(&ws), true, false, false, None)?;
    let capped_json: serde_json::Value = serde_json::from_str(&capped)?;
    assert_eq!(capped_json["count"], 100);

    let custom = sessions_list_text(Some(&ws), true, false, false, Some(2))?;
    let custom_json: serde_json::Value = serde_json::from_str(&custom)?;
    assert_eq!(custom_json["count"], 2);

    let all = sessions_list_text(Some(&ws), true, false, false, Some(0))?;
    let all_json: serde_json::Value = serde_json::from_str(&all)?;
    assert_eq!(all_json["count"], 105);

    clear_env("KAIZEN_HOME");
    clear_env("HOME");
    Ok(())
}

#[test]
fn load_defaults_to_registered_workspaces() -> anyhow::Result<()> {
    let _guard = env_lock().lock().unwrap_or_else(|e| e.into_inner());
    let home = TempDir::new()?;
    let ws1 = home.path().join("repo-a");
    let ws2 = home.path().join("repo-b");
    std::fs::create_dir_all(&ws1)?;
    std::fs::create_dir_all(&ws2)?;
    let ws1 = std::fs::canonicalize(ws1)?;
    let ws2 = std::fs::canonicalize(ws2)?;
    set_env("HOME", home.path());
    set_env("KAIZEN_HOME", home.path().join(".kaizen"));
    kaizen::core::workspace::resolve(Some(&ws1))?;
    kaizen::core::workspace::resolve(Some(&ws2))?;
    write_codex_session(home.path(), &ws1, "codex-load-1")?;
    write_claude_session(home.path(), &ws2, "claude-load-1")?;

    let text = load_text(None, true)?;
    let report: serde_json::Value = serde_json::from_str(&text)?;
    assert_eq!(report["workspace_count"], 2);
    assert_eq!(report["totals"]["sessions_upserted"], 2);

    let ws1_sessions: serde_json::Value =
        serde_json::from_str(&sessions_list_text(Some(&ws1), true, false, false, None)?)?;
    let ws2_sessions: serde_json::Value =
        serde_json::from_str(&sessions_list_text(Some(&ws2), true, false, false, None)?)?;
    assert_eq!(ws1_sessions["sessions"][0]["agent"], "codex");
    assert_eq!(ws2_sessions["sessions"][0]["agent"], "claude");

    clear_env("KAIZEN_HOME");
    clear_env("HOME");
    Ok(())
}

#[test]
fn load_workspace_limits_scope() -> anyhow::Result<()> {
    let _guard = env_lock().lock().unwrap_or_else(|e| e.into_inner());
    let home = TempDir::new()?;
    let ws1 = home.path().join("repo-a");
    let ws2 = home.path().join("repo-b");
    std::fs::create_dir_all(&ws1)?;
    std::fs::create_dir_all(&ws2)?;
    let ws1 = std::fs::canonicalize(ws1)?;
    let ws2 = std::fs::canonicalize(ws2)?;
    set_env("HOME", home.path());
    set_env("KAIZEN_HOME", home.path().join(".kaizen"));
    write_codex_session(home.path(), &ws1, "codex-load-scope")?;
    write_claude_session(home.path(), &ws2, "claude-load-scope")?;

    let text = load_text(Some(&ws1), true)?;
    let report: serde_json::Value = serde_json::from_str(&text)?;
    assert_eq!(report["workspace_count"], 1);
    assert_eq!(report["totals"]["sessions_upserted"], 1);
    let other: serde_json::Value =
        serde_json::from_str(&sessions_list_text(Some(&ws2), true, false, false, None)?)?;
    assert_eq!(other["count"], 0);

    clear_env("KAIZEN_HOME");
    clear_env("HOME");
    Ok(())
}

#[test]
fn cli_load_and_sessions_load_both_work() -> anyhow::Result<()> {
    let _guard = env_lock().lock().unwrap_or_else(|e| e.into_inner());
    let home = TempDir::new()?;
    let ws = home.path().join("repo");
    std::fs::create_dir_all(&ws)?;
    let ws = std::fs::canonicalize(ws)?;
    set_env("HOME", home.path());
    set_env("KAIZEN_HOME", home.path().join(".kaizen"));
    write_codex_session(home.path(), &ws, "codex-cli-load")?;

    let top = run_load_cmd(home.path(), &ws, &["load", "--workspace"])?;
    let nested = run_load_cmd(home.path(), &ws, &["sessions", "load", "--workspace"])?;
    assert_eq!(top["totals"]["sessions_upserted"], 1);
    assert_eq!(nested["totals"]["sessions_upserted"], 1);

    clear_env("KAIZEN_HOME");
    clear_env("HOME");
    Ok(())
}

fn run_load_cmd(home: &Path, ws: &Path, prefix: &[&str]) -> anyhow::Result<serde_json::Value> {
    let mut args = prefix.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    args.push(ws.to_string_lossy().to_string());
    args.push("--json".into());
    let out = Command::new(env!("CARGO_BIN_EXE_kaizen"))
        .args(args)
        .env("HOME", home)
        .env("KAIZEN_HOME", home.join(".kaizen"))
        .output()?;
    anyhow::ensure!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    Ok(serde_json::from_slice(&out.stdout)?)
}
