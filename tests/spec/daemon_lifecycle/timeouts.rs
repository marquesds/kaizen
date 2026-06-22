// SPDX-License-Identifier: AGPL-3.0-or-later
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub(super) fn hung_daemon_ipc_times_out_without_starting_duplicate() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join(".kaizen");
    let _server = HungSocket::start(&home);
    for args in lifecycle_commands() {
        assert_bounded_timeout(run_bounded(&home, args));
    }
    assert!(!home.join("daemon.pid").exists());
    assert!(home.join("daemon.sock").exists());
}

fn lifecycle_commands() -> [&'static [&'static str]; 4] {
    [
        &["daemon", "status"],
        &["daemon", "stop"],
        &["daemon", "restart"],
        &["daemon", "start", "--background"],
    ]
}

struct TimedOutput {
    timed_out: bool,
    output: Output,
}

fn run_bounded(home: &Path, args: &[&str]) -> TimedOutput {
    let mut child = command(home, args).spawn().unwrap();
    let timed_out = wait_deadline(&mut child, Duration::from_millis(3_000));
    if timed_out {
        child.kill().unwrap();
    }
    let output = child.wait_with_output().unwrap();
    TimedOutput { timed_out, output }
}

fn command(home: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_kaizen"));
    command.args(args).env("KAIZEN_HOME", home);
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    command
}

fn wait_deadline(child: &mut std::process::Child, duration: Duration) -> bool {
    let deadline = Instant::now() + duration;
    while Instant::now() < deadline {
        if child.try_wait().unwrap().is_some() {
            return false;
        }
        thread::sleep(Duration::from_millis(10));
    }
    true
}

fn assert_bounded_timeout(result: TimedOutput) {
    let stderr = String::from_utf8_lossy(&result.output.stderr);
    assert!(!result.timed_out, "command exceeded process deadline");
    assert!(
        !result.output.status.success(),
        "command unexpectedly succeeded"
    );
    assert!(stderr.contains("daemon IPC timed out"), "{stderr}");
}

struct HungSocket {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    path: PathBuf,
}

impl HungSocket {
    fn start(home: &Path) -> Self {
        std::fs::create_dir_all(home).unwrap();
        let path = home.join("daemon.sock");
        let listener = bind_listener(&path);
        let stop = Arc::new(AtomicBool::new(false));
        let thread = spawn_server(listener, stop.clone());
        Self {
            stop,
            thread: Some(thread),
            path,
        }
    }
}

fn bind_listener(path: &Path) -> UnixListener {
    let listener = UnixListener::bind(path).unwrap();
    listener.set_nonblocking(true).unwrap();
    listener
}

impl Drop for HungSocket {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.thread.take().unwrap().join().unwrap();
        let _ = std::fs::remove_file(&self.path);
    }
}

fn spawn_server(listener: UnixListener, stop: Arc<AtomicBool>) -> JoinHandle<()> {
    thread::spawn(move || hold_connections(listener, stop))
}

fn hold_connections(listener: UnixListener, stop: Arc<AtomicBool>) {
    let mut streams = Vec::new();
    while !stop.load(Ordering::Relaxed) {
        accept_or_wait(&listener, &mut streams);
    }
}

fn accept_or_wait(listener: &UnixListener, streams: &mut Vec<UnixStream>) {
    match listener.accept() {
        Ok((stream, _)) => streams.push(stream),
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
            thread::sleep(Duration::from_millis(5));
        }
        Err(error) => panic!("accept hung daemon client: {error}"),
    }
}
