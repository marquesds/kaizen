// SPDX-License-Identifier: AGPL-3.0-or-later
use quint_connect::*;
use std::io::Write;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

struct DaemonHandshakeDriver;

impl Driver for DaemonHandshakeDriver {
    type State = ();

    fn step(&mut self, _step: &Step) -> Result {
        Ok(())
    }
}

#[quint_run(spec = "specs/daemon-handshake.qnt", max_samples = 20, max_steps = 8)]
fn daemon_handshake_run() -> impl Driver {
    DaemonHandshakeDriver
}

#[test]
fn daemon_spawn_ingest_query_stop() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let workspace = tmp.path().join("repo");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&workspace).unwrap();

    let bin = env!("CARGO_BIN_EXE_kaizen");
    let mut daemon = Command::new(bin)
        .args(["daemon", "start", "--background"])
        .env("KAIZEN_HOME", home.join(".kaizen"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    wait_ok(bin, &home, ["daemon", "status"]);

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
    let _ = daemon.wait();
}

fn wait_ok<const N: usize>(bin: &str, home: &std::path::Path, args: [&str; N]) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        let output = Command::new(bin)
            .args(args)
            .env("KAIZEN_HOME", home.join(".kaizen"))
            .output()
            .unwrap();
        if output.status.success() {
            return;
        }
        thread::sleep(Duration::from_millis(25));
    }
    panic!("daemon did not become ready");
}
