// SPDX-License-Identifier: AGPL-3.0-or-later
//! Pure parsers for `cargo test` / `cargo clippy` text; `run_outcome_measure` at I/O boundary.

use regex::Regex;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Counts from `cargo test` output (final `test result:` line if present).
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub struct CargoTestCounts {
    pub passed: i32,
    pub failed: i32,
    pub ignored: i32,
}

/// Parse `test result: ok. 3 passed; 0 failed; 1 ignored; ...` from combined output.
pub fn parse_cargo_test_summary(text: &str) -> Option<CargoTestCounts> {
    let re =
        Regex::new(r"test result:\s*\w+\.\s*(\d+)\s+passed;\s*(\d+)\s+failed;\s*(\d+)\s+ignored")
            .ok()?;
    let cap = re.captures(text)?;
    Some(CargoTestCounts {
        passed: cap.get(1)?.as_str().parse().ok()?,
        failed: cap.get(2)?.as_str().parse().ok()?,
        ignored: cap.get(3)?.as_str().parse().ok()?,
    })
}

/// Heuristic: lines starting with `error:` in clippy/build output.
pub fn parse_clippy_error_count(text: &str) -> i32 {
    text.lines()
        .filter(|l| l.trim_start().starts_with("error:"))
        .count() as i32
}

/// Result of a workspace outcome run (for DB upsert).
#[derive(Debug, Default)]
pub struct OutcomeMeasureResult {
    pub test_passed: Option<i32>,
    pub test_failed: Option<i32>,
    pub test_skipped: Option<i32>,
    pub lint_errors: Option<i32>,
    pub measure_error: Option<String>,
}

struct Captured {
    combined: String,
    exit_fail: bool,
    timed_out: bool,
}

/// Run `test_cmd` (shell) under `workspace` with `timeout`, optional `lint_cmd` after.
pub fn run_outcome_measure(
    workspace: &Path,
    test_cmd: &str,
    lint_cmd: Option<&str>,
    timeout: Duration,
) -> OutcomeMeasureResult {
    let test = shell_output(workspace, test_cmd, timeout);
    let mut m = to_measure_result(&test);
    if let Some(lc) = lint_cmd.filter(|s| !s.is_empty()) {
        let lint = shell_output(workspace, lc, timeout);
        m.lint_errors = Some(parse_clippy_error_count(&lint.combined));
        if lint.timed_out {
            m.measure_error = m.measure_error.or(Some("lint command timed out".into()));
        } else if lint.exit_fail {
            m.measure_error = m.measure_error.or(Some("lint command failed".into()));
        }
    }
    m
}

fn to_measure_result(cap: &Captured) -> OutcomeMeasureResult {
    let counts = parse_cargo_test_summary(&cap.combined);
    let (tp, tf, tsk) = match counts {
        Some(c) => (Some(c.passed), Some(c.failed), Some(c.ignored)),
        None => (None, None, None),
    };
    let mut err = cap.then_error();
    if err.is_none() && cap.exit_fail && counts.is_none() {
        err = Some("command failed (no test result line)".into());
    }
    OutcomeMeasureResult {
        test_passed: tp,
        test_failed: tf,
        test_skipped: tsk,
        lint_errors: None,
        measure_error: err,
    }
}

impl Captured {
    fn then_error(&self) -> Option<String> {
        if self.timed_out {
            Some("command timed out".into())
        } else {
            None
        }
    }
}

fn shell_output(workspace: &Path, cmd: &str, timeout: Duration) -> Captured {
    let sh = if cfg!(unix) { "/bin/sh" } else { "sh" };
    let mut c = match Command::new(sh)
        .arg("-c")
        .arg(cmd)
        .current_dir(workspace)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(ch) => ch,
        Err(_) => {
            return Captured {
                combined: String::new(),
                exit_fail: true,
                timed_out: false,
            };
        }
    };
    let status = wait_limited(&mut c, timeout);
    let timed_out = status.is_err();
    let mut combined = String::new();
    if let Some(mut stdout) = c.stdout.take() {
        let _ = stdout.read_to_string(&mut combined);
    }
    if let Some(mut stderr) = c.stderr.take() {
        let _ = stderr.read_to_string(&mut combined);
    }
    let exit_ok = status.as_ref().map(|s| s.success()).unwrap_or(false);
    let exit_fail = timed_out || !exit_ok;
    Captured {
        combined,
        exit_fail,
        timed_out,
    }
}

fn wait_limited(
    child: &mut std::process::Child,
    timeout: Duration,
) -> Result<std::process::ExitStatus, ()> {
    let start = Instant::now();
    while start.elapsed() < timeout {
        match child.try_wait() {
            Ok(Some(s)) => return Ok(s),
            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
            Err(_) => return child.wait().map_err(|_| ()),
        }
    }
    let _ = child.kill();
    child.wait().map_err(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_test_result_line() {
        let t = "foo\n\ntest result: ok. 2 passed; 0 failed; 1 ignored; 0 measured; blah\n";
        let c = parse_cargo_test_summary(t).unwrap();
        assert_eq!(c.passed, 2);
        assert_eq!(c.failed, 0);
        assert_eq!(c.ignored, 1);
    }

    #[test]
    fn clippy_errors_count() {
        let t = "error: use of moved value\n\nerror: aborting";
        assert_eq!(parse_clippy_error_count(t), 2);
    }
}
