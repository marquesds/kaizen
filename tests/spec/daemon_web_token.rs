// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

#[test]
fn daemon_reuses_private_web_token_after_restart() {
    let tmp = tempfile::tempdir().unwrap();
    let daemon = Daemon::new(tmp.path());
    let first = daemon.start();
    let token_path = daemon.home.join("web_token.hex");
    assert_eq!(std::fs::read_to_string(&token_path).unwrap(), first);
    assert_private(&token_path);
    assert_eq!(daemon.status_token(), first);
    daemon.stop();
    assert_eq!(daemon.start(), first);
}

struct Daemon {
    home: PathBuf,
}

impl Daemon {
    fn new(root: &Path) -> Self {
        Self {
            home: root.join(".kaizen"),
        }
    }

    fn start(&self) -> String {
        let output = self.run(["daemon", "start", "--background"]);
        assert_success(&output);
        web_token(&output.stdout)
    }

    fn status_token(&self) -> String {
        let output = self.run(["daemon", "status"]);
        assert_success(&output);
        web_token(&output.stdout)
    }

    fn stop(&self) {
        assert_success(&self.run(["daemon", "stop"]));
    }

    fn run<const N: usize>(&self, args: [&str; N]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_kaizen"))
            .args(args)
            .env("KAIZEN_HOME", &self.home)
            .output()
            .unwrap()
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.run(["daemon", "stop"]);
    }
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn web_token(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .lines()
        .find_map(|line| line.strip_prefix("web: "))
        .and_then(|url| url.rsplit_once("token=").map(|(_, token)| token.into()))
        .expect("web URL with token")
}

#[cfg(unix)]
fn assert_private(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
}

#[cfg(not(unix))]
fn assert_private(_path: &Path) {}
