// SPDX-License-Identifier: AGPL-3.0-or-later
use std::thread;
use std::time::{Duration, Instant};

use super::support::{TestDaemon, assert_success, text};

pub(super) fn restart_restores_registered_capture() {
    let daemon = TestDaemon::new();
    assert_success(&daemon.run(&["init", "--workspace", daemon.workspace_str()]));
    daemon.write_codex_session("restart-session");
    assert_success(&daemon.run(&["daemon", "restart"]));
    assert_restored(&daemon);
}

pub(super) fn stopped_daemon_restart_starts_background() {
    let daemon = TestDaemon::new();
    let output = daemon.run(&["daemon", "restart"]);
    assert_success(&output);
    assert!(
        text(&output).contains("daemon started"),
        "{}",
        text(&output)
    );
    assert!(text(&daemon.run(&["daemon", "status"])).contains("status: running"));
}

fn assert_restored(daemon: &TestDaemon) {
    let status = text(&daemon.run(&["daemon", "status"]));
    assert!(
        status.contains(&format!("capture: {}", daemon.workspace_str())),
        "{status}"
    );
    assert!(status.contains("  deep: false"), "{status}");
    assert!(status.contains("  watchers: 1"), "{status}");
    assert!(wait_for_session(daemon, "restart-session"));
}

fn wait_for_session(daemon: &TestDaemon, id: &str) -> bool {
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        let args = ["sessions", "list", "--workspace", daemon.workspace_str()];
        if text(&daemon.run(&args)).contains(id) {
            return true;
        }
        thread::sleep(Duration::from_millis(25));
    }
    false
}
