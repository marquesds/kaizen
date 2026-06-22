// SPDX-License-Identifier: AGPL-3.0-or-later
use rusqlite::Connection;
use std::process::{Child, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use super::support::{TestDaemon, assert_success, text};

pub(super) fn startup_waits_for_registered_store_migration() {
    let daemon = TestDaemon::new();
    init_without_daemon(&daemon, daemon.workspace_str());
    let (connection, db) = lock_workspace_db(&daemon);
    let mut start = spawn_restart(&daemon);
    let blocked = stays_running(&mut start, Duration::from_millis(300));
    connection.execute_batch("ROLLBACK").unwrap();
    assert_success(&start.wait_with_output().unwrap());
    assert!(blocked, "daemon reported ready before migration completed");
    assert!(table_exists(&db, "session_search_prompts"));
}

pub(super) fn startup_readiness_allows_slow_registered_migration() {
    let daemon = TestDaemon::new();
    init_without_daemon(&daemon, daemon.workspace_str());
    let (connection, _) = lock_workspace_db(&daemon);
    let mut start = spawn_restart(&daemon);
    let blocked = stays_running(&mut start, Duration::from_millis(2_300));
    connection.execute_batch("ROLLBACK").unwrap();
    assert_ready(start.wait_with_output().unwrap());
    assert!(blocked, "daemon stopped waiting before migration completed");
}

pub(super) fn scan_failure_status_recovers_without_blocking_other_workspaces() {
    let daemon = TestDaemon::new();
    let bad = daemon.create_workspace("bad-repo");
    register_workspaces(&daemon, &bad);
    poison_db(&daemon, &bad);
    restart_with_warning_logs(&daemon);
    assert_failure_visible(&daemon, &bad);
    recover_workspace(&daemon, &bad);
    assert_recovered(&daemon);
}

fn restart_with_warning_logs(daemon: &TestDaemon) {
    let output = daemon
        .command(&["daemon", "restart"])
        .env("RUST_LOG", "warn")
        .output()
        .unwrap();
    assert_success(&output);
}

fn register_workspaces(daemon: &TestDaemon, bad: &std::path::Path) {
    init_without_daemon(daemon, daemon.workspace_str());
    init_without_daemon(daemon, bad.to_str().unwrap());
}

fn recover_workspace(daemon: &TestDaemon, workspace: &std::path::Path) {
    std::fs::remove_file(daemon.db_path(workspace)).unwrap();
    let path = workspace.to_str().unwrap();
    assert_success(&daemon.run(&["init", "--workspace", path]));
}

fn init_without_daemon(daemon: &TestDaemon, workspace: &str) {
    assert_success(&daemon.run(&["--no-daemon", "init", "--workspace", workspace]));
}

fn assert_ready(output: std::process::Output) {
    assert_success(&output);
    let stdout = text(&output);
    assert!(stdout.contains("daemon started"), "{stdout}");
    assert!(stdout.contains("web: http://127.0.0.1:"), "{stdout}");
}

fn lock_workspace_db(daemon: &TestDaemon) -> (Connection, std::path::PathBuf) {
    let db = daemon.db_path(daemon.workspace());
    let connection = Connection::open(&db).unwrap();
    connection.execute_batch("BEGIN EXCLUSIVE").unwrap();
    (connection, db)
}

fn spawn_restart(daemon: &TestDaemon) -> Child {
    daemon
        .command(&["daemon", "restart"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap()
}

fn stays_running(child: &mut Child, duration: Duration) -> bool {
    let deadline = Instant::now() + duration;
    while Instant::now() < deadline {
        if child.try_wait().unwrap().is_some() {
            return false;
        }
        thread::sleep(Duration::from_millis(10));
    }
    true
}

fn table_exists(db: &std::path::Path, table: &str) -> bool {
    let connection = Connection::open(db).unwrap();
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
            [table],
            |row| row.get(0),
        )
        .unwrap()
}

fn poison_db(daemon: &TestDaemon, workspace: &std::path::Path) {
    let db = daemon.db_path(workspace);
    std::fs::create_dir_all(db.parent().unwrap()).unwrap();
    std::fs::write(db, "not sqlite").unwrap();
}

fn assert_failure_visible(daemon: &TestDaemon, bad: &std::path::Path) {
    let status = text(&daemon.run(&["daemon", "status"]));
    assert!(status.contains(daemon.workspace_str()), "{status}");
    assert!(status.contains(bad.to_str().unwrap()), "{status}");
    assert!(status.contains("errors: transcript-scanner:"), "{status}");
    assert_log_eventually_contains(
        &daemon.home().join(".kaizen/daemon.log"),
        "daemon scanner failed",
    );
}

fn assert_log_eventually_contains(path: &std::path::Path, needle: &str) {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let log = std::fs::read_to_string(path).unwrap_or_default();
        if log.contains(needle) {
            return;
        }
        assert!(Instant::now() < deadline, "{log}");
        thread::sleep(Duration::from_millis(10));
    }
}

fn assert_recovered(daemon: &TestDaemon) {
    let status = text(&daemon.run(&["daemon", "status"]));
    assert!(!status.contains("errors: transcript-scanner:"), "{status}");
}
