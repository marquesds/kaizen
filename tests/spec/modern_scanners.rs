// SPDX-License-Identifier: AGPL-3.0-or-later

use kaizen::collect::tail::claude_code::scan_claude_session_file;
use kaizen::collect::tail::codex_desktop::scan_codex_session_file;
use kaizen::core::event::EventKind;
use std::path::Path;

#[test]
fn scans_modern_codex_file() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/codex-modern/session.jsonl");
    let (record, events) = scan_codex_session_file(&path).unwrap();
    assert_eq!(record.id, "codex-modern-1");
    assert_eq!(record.workspace, "/tmp/kaizen-modern");
    assert_eq!(record.model.as_deref(), Some("gpt-4o"));
    assert!(events.iter().any(|e| e.kind == EventKind::ToolCall));
    assert!(events.iter().any(|e| e.kind == EventKind::ToolResult));
    assert!(events.iter().any(|e| e.kind == EventKind::Cost));
    assert!(events.iter().any(|e| e.cost_usd_e6.is_some_and(|c| c > 0)));
}

#[test]
fn scans_top_level_claude_file() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/claude-modern/session.jsonl");
    let (record, events) = scan_claude_session_file(&path, None, None).unwrap();
    assert_eq!(record.id, "claude-modern-1");
    assert_eq!(record.workspace, "/tmp/kaizen-modern");
    assert_eq!(record.model.as_deref(), Some("claude-sonnet-4"));
    assert!(events.iter().any(|e| e.kind == EventKind::ToolCall));
    assert!(events.iter().any(|e| e.kind == EventKind::ToolResult));
}
