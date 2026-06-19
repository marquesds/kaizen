// SPDX-License-Identifier: AGPL-3.0-or-later

use super::*;
use crate::core::paths::test_lock;
use tempfile::TempDir;

fn setup_ws() -> (TempDir, TempDir) {
    let home = TempDir::new().unwrap();
    let workspace = TempDir::new().unwrap();
    unsafe { std::env::set_var("KAIZEN_HOME", home.path()) };
    (home, workspace)
}

fn sessions(workspace: &TempDir) -> Vec<SessionRecord> {
    let path = crate::core::workspace::db_path(workspace.path()).unwrap();
    let store = Store::open(&path).unwrap();
    store
        .list_sessions(workspace.path().to_string_lossy().as_ref())
        .unwrap()
}

#[test]
fn session_start_records_source_as_agent_not_unknown() {
    let _guard = test_lock::global().lock().unwrap();
    let (_home, workspace) = setup_ws();
    let payload =
        r#"{"hook_event_name":"SessionStart","session_id":"s-agent-1","source":"startup"}"#;
    ingest_hook_text(
        IngestSource::Claude,
        payload,
        Some(workspace.path().to_path_buf()),
    )
    .unwrap();
    let rows = sessions(&workspace);
    unsafe { std::env::remove_var("KAIZEN_HOME") };
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].agent, "claude");
}

#[test]
fn missing_timestamp_falls_back_to_now() {
    let _guard = test_lock::global().lock().unwrap();
    let (_home, workspace) = setup_ws();
    let payload = r#"{"hook_event_name":"SessionStart","session_id":"s-ts","source":"startup"}"#;
    ingest_hook_text(
        IngestSource::Claude,
        payload,
        Some(workspace.path().to_path_buf()),
    )
    .unwrap();
    let rows = sessions(&workspace);
    unsafe { std::env::remove_var("KAIZEN_HOME") };
    assert!(rows[0].started_at_ms > 0, "started_at_ms must not be 0");
}

#[test]
fn post_tool_use_without_session_start_auto_provisions_stub() {
    let _guard = test_lock::global().lock().unwrap();
    let (_home, workspace) = setup_ws();
    let payload = r#"{"event":"PostToolUse","session_id":"s-stub","tool_name":"Read","tool_input":{"file_path":"/tmp/x"},"tool_response":{"content":"hi"}}"#;
    ingest_hook_text(
        IngestSource::Cursor,
        payload,
        Some(workspace.path().to_path_buf()),
    )
    .unwrap();
    let rows = sessions(&workspace);
    unsafe { std::env::remove_var("KAIZEN_HOME") };
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].agent, "cursor");
    assert_eq!(rows[0].id, "s-stub");
}

#[test]
fn claude_hook_with_codex_evidence_records_codex_identity() {
    let _guard = test_lock::global().lock().unwrap();
    let (_home, workspace) = setup_ws();
    let payload = r#"{"hook_event_name":"SessionStart","session_id":"s-codex","turn_id":"t1","model":"gpt-5.4","transcript_path":"/tmp/.codex/sessions/s.jsonl"}"#;
    ingest_hook_text(IngestSource::Claude, payload, Some(workspace.path().into())).unwrap();
    let row = sessions(&workspace).remove(0);
    unsafe { std::env::remove_var("KAIZEN_HOME") };
    assert_eq!(row.agent, "codex");
    assert_eq!(row.model.as_deref(), Some("gpt-5.4"));
    assert_eq!(row.trace_path, "/tmp/.codex/sessions/s.jsonl");
}

#[test]
fn later_hook_enriches_missing_model() {
    let _guard = test_lock::global().lock().unwrap();
    let (_home, workspace) = setup_ws();
    let start = r#"{"hook_event_name":"SessionStart","session_id":"s-model"}"#;
    ingest_hook_text(IngestSource::Claude, start, Some(workspace.path().into())).unwrap();
    let call = r#"{"hook_event_name":"PreToolUse","session_id":"s-model","turn_id":"t1","model":"kindle-alpha","tool_name":"Bash"}"#;
    ingest_hook_text(IngestSource::Claude, call, Some(workspace.path().into())).unwrap();
    let result = r#"{"hook_event_name":"PostToolUse","session_id":"s-model","tool_name":"Bash"}"#;
    ingest_hook_text(IngestSource::Claude, result, Some(workspace.path().into())).unwrap();
    let row = sessions(&workspace).remove(0);
    unsafe { std::env::remove_var("KAIZEN_HOME") };
    assert_eq!(row.agent, "codex");
    assert_eq!(row.model.as_deref(), Some("kindle-alpha"));
}
