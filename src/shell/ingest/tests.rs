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
