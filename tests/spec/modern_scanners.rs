// SPDX-License-Identifier: AGPL-3.0-or-later

use kaizen::collect::tail::claude_code::scan_claude_session_file;
use kaizen::collect::tail::codex_desktop::scan_codex_session_file;
use kaizen::collect::tail::cursor_state_db::{
    read_items_with_prefix, scan_cursor_state_db_workspace,
};
use kaizen::collect::tail::{antigravity, gemini, kimi, pi};
use kaizen::core::event::EventKind;
use std::path::Path;

fn fixture(path: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(path)
}

#[test]
fn scans_modern_codex_file() {
    let path = fixture("tests/fixtures/codex-modern/session.jsonl");
    let (record, events) = scan_codex_session_file(&path).unwrap();
    assert_eq!(record.id, "codex-modern-1");
    assert_eq!(record.workspace, "/tmp/kaizen-modern");
    assert_eq!(record.model.as_deref(), Some("gpt-4o"));
    assert!(events.iter().any(|e| e.kind == EventKind::ToolCall));
    assert!(events.iter().any(|e| e.kind == EventKind::ToolResult));
    assert!(events.iter().any(|e| e.kind == EventKind::Cost));
    assert!(events.iter().any(|e| e.cost_usd_e6.is_some_and(|c| c > 0)));
    let cost = events.iter().find(|e| e.kind == EventKind::Cost).unwrap();
    assert_eq!(cost.context_used_tokens, Some(1600));
    assert_eq!(cost.context_max_tokens, Some(128000));
}

#[test]
fn scans_top_level_claude_file() {
    let path = fixture("tests/fixtures/claude-modern/session.jsonl");
    let (record, events) = scan_claude_session_file(&path, None, None).unwrap();
    assert_eq!(record.id, "claude-modern-1");
    assert_eq!(record.workspace, "/tmp/kaizen-modern");
    assert_eq!(record.model.as_deref(), Some("claude-sonnet-4"));
    assert!(events.iter().any(|e| e.kind == EventKind::ToolCall));
    assert!(events.iter().any(|e| e.kind == EventKind::ToolResult));
}

#[test]
fn scans_gemini_file_and_skips_bad_records() {
    let path = fixture("tests/fixtures/gemini-modern/session.jsonl");
    let (record, events) = gemini::scan_gemini_session_file(&path).unwrap();
    assert_eq!(record.id, "gemini-modern-1");
    assert_eq!(record.workspace, "/tmp/kaizen-modern");
    assert_eq!(record.model.as_deref(), Some("gemini-2.5-pro"));
    assert!(events.iter().any(|e| e.kind == EventKind::ToolCall));
    assert!(events.iter().any(|e| e.kind == EventKind::ToolResult));
}

#[test]
fn scans_pi_file_and_skips_bad_records() {
    let path = fixture("tests/fixtures/pi-modern/session.jsonl");
    let (record, events) = pi::scan_pi_session_file(&path).unwrap();
    assert_eq!(record.id, "pi-modern-1");
    assert_eq!(record.workspace, "/tmp/kaizen-modern");
    assert_eq!(record.model.as_deref(), Some("pi-agent"));
    assert!(events.iter().any(|e| e.tool.as_deref() == Some("search")));
    assert!(events.iter().any(|e| e.kind == EventKind::ToolResult));
}

#[test]
fn scans_kimi_file_and_skips_bad_records() {
    let path = fixture("tests/fixtures/kimi-modern/session.jsonl");
    let (record, events) = kimi::scan_kimi_session_file(&path).unwrap();
    assert_eq!(record.id, "kimi-modern-1");
    assert_eq!(record.workspace, "/tmp/kaizen-modern");
    assert_eq!(record.model.as_deref(), Some("kimi-k2"));
    assert!(events.iter().any(|e| e.tool.as_deref() == Some("shell")));
    assert!(events.iter().any(|e| e.kind == EventKind::ToolResult));
}

#[test]
fn scans_antigravity_file_and_skips_bad_records() {
    let path = fixture("tests/fixtures/antigravity-modern/session.jsonl");
    let (record, events) = antigravity::scan_antigravity_session_file(&path).unwrap();
    assert_eq!(record.id, "antigravity-modern-1");
    assert_eq!(record.workspace, "/tmp/kaizen-modern");
    assert_eq!(record.model.as_deref(), Some("gemini-3-pro"));
    assert!(
        events
            .iter()
            .any(|e| e.tool.as_deref() == Some("edit_file"))
    );
    assert!(events.iter().any(|e| e.kind == EventKind::ToolResult));
}

#[test]
fn reads_cursor_state_db_values_read_only() {
    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path().join("repo");
    std::fs::create_dir_all(workspace.join(".cursor")).unwrap();
    let db = workspace.join(".cursor/state.vscdb");
    let conn = rusqlite::Connection::open(&db).unwrap();
    conn.execute(
        "CREATE TABLE ItemTable (key TEXT PRIMARY KEY, value BLOB)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO ItemTable (key, value) VALUES (?1, ?2)",
        (
            "composerData:one",
            serde_json::json!({
                "id":"cursor-state-1",
                "workspace": workspace,
                "model":"cursor-agent",
                "timestamp_ms":1780000040000_u64
            })
            .to_string(),
        ),
    )
    .unwrap();
    conn.execute(
        "INSERT INTO ItemTable (key, value) VALUES (?1, ?2)",
        ("other", "ignored"),
    )
    .unwrap();
    drop(conn);

    let rows = read_items_with_prefix(&db, "composerData:").unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].key, "composerData:one");
    let sessions = scan_cursor_state_db_workspace(&workspace);
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].0.id, "cursor-state-1");
    assert_eq!(sessions[0].1.len(), 1);
}
