// SPDX-License-Identifier: AGPL-3.0-or-later
use quint_connect::*;
use serde::Deserialize;
use std::io::Write;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Eq, PartialEq, Deserialize)]
struct DaemonHandshakeState {
    server_running: bool,
    #[serde(with = "itf::de::As::<itf::de::Integer>")]
    client_version: i32,
    subscribed: bool,
    #[serde(with = "itf::de::As::<itf::de::Integer>")]
    queue_depth: i32,
    stopped: bool,
    background_returned: bool,
    status_result: String,
}

#[derive(Debug)]
struct DaemonHandshakeDriver {
    server_running: bool,
    client_version: i32,
    subscribed: bool,
    queue_depth: i32,
    stopped: bool,
    background_returned: bool,
    status_result: String,
}

impl Default for DaemonHandshakeDriver {
    fn default() -> Self {
        Self {
            server_running: false,
            client_version: 1,
            subscribed: false,
            queue_depth: 0,
            stopped: false,
            background_returned: false,
            status_result: "none".into(),
        }
    }
}

impl State<DaemonHandshakeDriver> for DaemonHandshakeState {
    fn from_driver(d: &DaemonHandshakeDriver) -> Result<Self> {
        Ok(Self {
            server_running: d.server_running,
            client_version: d.client_version,
            subscribed: d.subscribed,
            queue_depth: d.queue_depth,
            stopped: d.stopped,
            background_returned: d.background_returned,
            status_result: d.status_result.clone(),
        })
    }
}

impl Driver for DaemonHandshakeDriver {
    type State = DaemonHandshakeState;

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => *self = Self::default(),
            step => *self = Self::default(),
            start => {
                self.server_running = true;
                self.stopped = false;
            },
            start_background => {
                self.server_running = true;
                self.stopped = false;
                self.background_returned = true;
            },
            bad_version => {
                if !self.server_running {
                    anyhow::bail!("bad_version not enabled");
                }
                self.client_version = 0;
            },
            retry_supported => {
                if !self.server_running || self.client_version == 1 {
                    anyhow::bail!("retry_supported not enabled");
                }
                self.client_version = 1;
            },
            subscribe => {
                if !self.server_running || self.client_version != 1 {
                    anyhow::bail!("subscribe not enabled");
                }
                self.subscribed = true;
            },
            unsubscribe => {
                if !self.subscribed {
                    anyhow::bail!("unsubscribe not enabled");
                }
                self.subscribed = false;
            },
            stop => {
                if !self.server_running || self.queue_depth != 0 {
                    anyhow::bail!("stop not enabled");
                }
                self.server_running = false;
                self.subscribed = false;
                self.stopped = true;
                self.queue_depth = 0;
                self.background_returned = false;
            },
            status_running => {
                if !self.server_running || self.client_version != 1 {
                    anyhow::bail!("status_running not enabled");
                }
                self.status_result = "running".into();
            },
            status_stopped => {
                if self.server_running {
                    anyhow::bail!("status_stopped not enabled");
                }
                self.status_result = "stopped".into();
            },
            status_protocol_error => {
                if !self.server_running || self.client_version == 1 {
                    anyhow::bail!("status_protocol_error not enabled");
                }
                self.status_result = "error".into();
            },
        })
    }
}

#[quint_run(spec = "specs/daemon-handshake.qnt", max_samples = 20, max_steps = 8)]
fn daemon_handshake_run() -> impl Driver {
    DaemonHandshakeDriver::default()
}

#[test]
fn daemon_spawn_ingest_query_stop() {
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

#[test]
fn init_starts_daemon_capture_status() {
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
