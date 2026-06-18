// SPDX-License-Identifier: AGPL-3.0-or-later
use std::io::Write;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

pub(super) fn daemon_spawn_ingest_query_stop() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let workspace = tmp.path().join("repo");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&workspace).unwrap();

    let bin = env!("CARGO_BIN_EXE_kaizen");
    let start = Command::new(bin)
        .args(["daemon", "start", "--background"])
        .env("KAIZEN_HOME", home.join(".kaizen"))
        .output()
        .unwrap();
    assert!(
        start.status.success(),
        "{}",
        String::from_utf8_lossy(&start.stderr)
    );
    let start_stdout = String::from_utf8_lossy(&start.stdout);
    assert!(start_stdout.contains("pid:"), "{start_stdout}");
    assert!(
        start_stdout.contains("web: http://127.0.0.1:"),
        "{start_stdout}"
    );
    let status_stdout = wait_ok(bin, &home, ["daemon", "status"]);
    assert!(
        status_stdout.contains("web: http://127.0.0.1:"),
        "{status_stdout}"
    );

    let payload =
        r#"{"event":"SessionStart","session_id":"daemon-s1","timestamp_ms":1714000000000}"#;
    let mut ingest = Command::new(bin)
        .args([
            "ingest",
            "hook",
            "--source",
            "cursor",
            "--workspace",
            workspace.to_str().unwrap(),
        ])
        .env("KAIZEN_HOME", home.join(".kaizen"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    ingest
        .stdin
        .as_mut()
        .unwrap()
        .write_all(payload.as_bytes())
        .unwrap();
    let ingest_out = ingest.wait_with_output().unwrap();
    assert!(
        ingest_out.status.success(),
        "{}",
        String::from_utf8_lossy(&ingest_out.stderr)
    );

    let list = Command::new(bin)
        .args([
            "sessions",
            "list",
            "--workspace",
            workspace.to_str().unwrap(),
        ])
        .env("KAIZEN_HOME", home.join(".kaizen"))
        .output()
        .unwrap();
    assert!(
        list.status.success(),
        "{}",
        String::from_utf8_lossy(&list.stderr)
    );
    assert!(String::from_utf8_lossy(&list.stdout).contains("daemon-s1"));

    let stop = Command::new(bin)
        .args(["daemon", "stop"])
        .env("KAIZEN_HOME", home.join(".kaizen"))
        .output()
        .unwrap();
    assert!(
        stop.status.success(),
        "{}",
        String::from_utf8_lossy(&stop.stderr)
    );

    let status = Command::new(bin)
        .args(["daemon", "status"])
        .env("KAIZEN_HOME", home.join(".kaizen"))
        .output()
        .unwrap();
    assert!(
        status.status.success(),
        "{}",
        String::from_utf8_lossy(&status.stderr)
    );
    let stdout = String::from_utf8_lossy(&status.stdout);
    assert!(stdout.contains("status: stopped"), "{stdout}");
    assert!(stdout.contains("socket:"), "{stdout}");
}

pub(super) fn init_starts_daemon_capture_status() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let workspace = tmp.path().join("repo");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&workspace).unwrap();

    let bin = env!("CARGO_BIN_EXE_kaizen");
    let init = Command::new(bin)
        .args(["init", "--workspace", workspace.to_str().unwrap()])
        .env("HOME", &home)
        .env("KAIZEN_HOME", home.join(".kaizen"))
        .output()
        .unwrap();
    assert!(
        init.status.success(),
        "{}",
        String::from_utf8_lossy(&init.stderr)
    );
    let stdout = String::from_utf8_lossy(&init.stdout);
    assert!(stdout.contains("daemon capture"), "{stdout}");

    let status = Command::new(bin)
        .args(["daemon", "status"])
        .env("HOME", &home)
        .env("KAIZEN_HOME", home.join(".kaizen"))
        .output()
        .unwrap();
    assert!(
        status.status.success(),
        "{}",
        String::from_utf8_lossy(&status.stderr)
    );
    let stdout = String::from_utf8_lossy(&status.stdout);
    assert!(stdout.contains("capture:"), "{stdout}");
    assert!(stdout.contains("watchers:"), "{stdout}");

    let _ = Command::new(bin)
        .args(["daemon", "stop"])
        .env("HOME", &home)
        .env("KAIZEN_HOME", home.join(".kaizen"))
        .output();
}

fn wait_ok<const N: usize>(bin: &str, home: &std::path::Path, args: [&str; N]) -> String {
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        let output = Command::new(bin)
            .args(args)
            .env("KAIZEN_HOME", home.join(".kaizen"))
            .output()
            .unwrap();
        if output.status.success() {
            return String::from_utf8_lossy(&output.stdout).to_string();
        }
        thread::sleep(Duration::from_millis(25));
    }
    panic!("daemon did not become ready");
}
