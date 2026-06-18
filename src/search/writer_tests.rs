use super::*;

#[test]
fn sparse_hooks_do_not_commit_every_ten_seconds() {
    let dir = tempfile::tempdir().unwrap();
    let mut writer = PendingWriter::open(dir.path()).unwrap();
    writer.pending = 1;
    writer.last_commit = Instant::now() - Duration::from_secs(10);
    assert!(!writer.should_commit());
}

#[test]
fn active_session_commits_after_one_minute() {
    let dir = tempfile::tempdir().unwrap();
    let mut writer = PendingWriter::open(dir.path()).unwrap();
    writer.pending = 1;
    writer.last_commit = Instant::now() - Duration::from_secs(BATCH_SECS);
    assert!(writer.should_commit());
}

#[test]
fn full_batch_commits_without_waiting() {
    let dir = tempfile::tempdir().unwrap();
    let mut writer = PendingWriter::open(dir.path()).unwrap();
    writer.pending = BATCH_DOCS;
    assert!(writer.should_commit());
}
