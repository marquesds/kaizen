use super::MAX_RECENT_TRANSCRIPTS;
use serde_json::json;
use std::fs::{FileTimes, OpenOptions};
use std::path::Path;
use std::time::{Duration, UNIX_EPOCH};

#[test]
fn claude_scan_caps_recent_transcripts() {
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path().join("repo");
    let project = temp.path().join("claude");
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::create_dir_all(&project).unwrap();
    write_claude_rows(&project, &workspace, 40);

    let rows = super::claude_code::scan_claude_project_dir(&project, &workspace).unwrap();

    assert_eq!(rows.len(), MAX_RECENT_TRANSCRIPTS);
}

#[test]
fn codex_scan_caps_recent_transcripts() {
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path().join("repo");
    let root = temp.path().join("codex/2026/06/18");
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::create_dir_all(&root).unwrap();
    write_codex_rows(&root, &workspace, 40);

    let rows = super::codex_desktop::scan_codex_sessions_root(&root, &workspace).unwrap();

    assert_eq!(rows.len(), MAX_RECENT_TRANSCRIPTS);
}

#[test]
fn recent_path_budget_excludes_unchanged_files() {
    let temp = tempfile::tempdir().unwrap();
    let stale = temp.path().join("stale.jsonl");
    let recent = temp.path().join("recent.jsonl");
    std::fs::write(&stale, "{}").unwrap();
    std::fs::write(&recent, "{}").unwrap();
    set_modified(&stale, 1);
    set_modified(&recent, 3);

    let rows = super::newest_paths_since(vec![stale, recent.clone()], 2_000);

    assert_eq!(rows, vec![recent]);
}

#[test]
fn transcript_reader_excludes_large_old_prefix() {
    let temp = tempfile::tempdir().unwrap();
    let transcript = temp.path().join("large.jsonl");
    std::fs::write(&transcript, "{}\n".repeat(800_000)).unwrap();

    let (first_seq, content) = super::read_recent_jsonl(&transcript).unwrap();

    assert!(first_seq > 0);
    assert!(content.len() <= super::MAX_TRANSCRIPT_READ_BYTES as usize);
}

fn write_claude_rows(dir: &Path, workspace: &Path, count: usize) {
    (0..count).for_each(|index| {
        let row = json!({
            "type":"user", "timestamp":"2026-06-18T00:00:00Z",
            "cwd":workspace, "sessionId":format!("claude-{index:02}")
        });
        std::fs::write(dir.join(format!("{index:02}.jsonl")), row.to_string()).unwrap();
    });
}

fn write_codex_rows(dir: &Path, workspace: &Path, count: usize) {
    (0..count).for_each(|index| {
        let row = json!({
            "type":"session_meta", "timestamp":"2026-06-18T00:00:00Z",
            "payload":{"id":format!("codex-{index:02}"), "cwd":workspace}
        });
        std::fs::write(dir.join(format!("{index:02}.jsonl")), row.to_string()).unwrap();
    });
}

fn set_modified(path: &Path, seconds: u64) {
    let file = OpenOptions::new().write(true).open(path).unwrap();
    let times = FileTimes::new().set_modified(UNIX_EPOCH + Duration::from_secs(seconds));
    file.set_times(times).unwrap();
}
