// SPDX-License-Identifier: AGPL-3.0-or-later
use quint_connect::*;
use serde::Deserialize;
use serde_json::Value;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

#[derive(Debug, Eq, PartialEq, Deserialize)]
struct SessionTreeState {
    phase: String,
    session_exists: bool,
    #[serde(with = "itf::de::As::<itf::de::Integer>")]
    span_count: i32,
    rendered: String,
}

#[derive(Debug)]
struct SessionTreeDriver {
    phase: String,
    session_exists: bool,
    span_count: i32,
    rendered: String,
}

impl Default for SessionTreeDriver {
    fn default() -> Self {
        Self {
            phase: "Start".into(),
            session_exists: false,
            span_count: 0,
            rendered: "none".into(),
        }
    }
}

impl State<SessionTreeDriver> for SessionTreeState {
    fn from_driver(d: &SessionTreeDriver) -> Result<Self> {
        Ok(Self {
            phase: d.phase.clone(),
            session_exists: d.session_exists,
            span_count: d.span_count,
            rendered: d.rendered.clone(),
        })
    }
}

impl Driver for SessionTreeDriver {
    type State = SessionTreeState;

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => *self = Self::default(),
            step => *self = Self::default(),
            lookup_missing => {
                self.expect_phase("Start")?;
                self.phase = "Missing".into();
                self.session_exists = false;
                self.span_count = 0;
            },
            lookup_empty => {
                self.expect_phase("Start")?;
                self.phase = "Ready".into();
                self.session_exists = true;
                self.span_count = 0;
            },
            lookup_spans => {
                self.expect_phase("Start")?;
                self.phase = "Ready".into();
                self.session_exists = true;
                self.span_count = 1;
            },
            render_text_missing => {
                self.expect_phase("Missing")?;
                self.phase = "Error".into();
                self.rendered = "error".into();
            },
            render_text_empty => {
                self.expect_render_ready(0)?;
                self.phase = "Rendered".into();
                self.rendered = "placeholder".into();
            },
            render_text_tree => {
                self.expect_render_ready(1)?;
                self.phase = "Rendered".into();
                self.rendered = "tree".into();
            },
            render_json_empty => {
                self.expect_render_ready(0)?;
                self.phase = "Rendered".into();
                self.rendered = "json_empty".into();
            },
            render_json_tree => {
                self.expect_render_ready(1)?;
                self.phase = "Rendered".into();
                self.rendered = "json_tree".into();
            },
        })
    }
}

impl SessionTreeDriver {
    fn expect_phase(&self, expected: &str) -> Result {
        if self.phase != expected {
            anyhow::bail!("expected {expected}, got {}", self.phase);
        }
        Ok(())
    }

    fn expect_render_ready(&self, min_spans: i32) -> Result {
        self.expect_phase("Ready")?;
        if !self.session_exists || self.span_count < min_spans {
            anyhow::bail!("render not enabled");
        }
        Ok(())
    }
}

#[quint_run(spec = "specs/session-tree.qnt", max_samples = 12, max_steps = 6)]
fn session_tree_run() -> impl Driver {
    SessionTreeDriver::default()
}

#[test]
fn empty_session_tree_has_text_placeholder_and_json_empty_array() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let home = tmp.path().join("home");
    let ws = tmp.path().join("repo");
    std::fs::create_dir_all(&home)?;
    std::fs::create_dir_all(&ws)?;
    let bin = env!("CARGO_BIN_EXE_kaizen");

    run(bin, &home, &ws, &["init", "--workspace", path(&ws)])?;
    ingest_start(bin, &home, &ws, "tree-empty")?;

    let text = output(
        bin,
        &home,
        &ws,
        &["--no-daemon", "sessions", "tree", "tree-empty"],
    )?;
    assert!(text.contains("(no tool spans for session tree-empty)"));

    let json = output(
        bin,
        &home,
        &ws,
        &["--no-daemon", "sessions", "tree", "tree-empty", "--json"],
    )?;
    let value: Value = serde_json::from_str(&json)?;
    assert_eq!(value, Value::Array(vec![]));
    Ok(())
}

#[test]
fn missing_session_tree_is_error() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let home = tmp.path().join("home");
    let ws = tmp.path().join("repo");
    std::fs::create_dir_all(&home)?;
    std::fs::create_dir_all(&ws)?;
    let bin = env!("CARGO_BIN_EXE_kaizen");

    run(bin, &home, &ws, &["init", "--workspace", path(&ws)])?;
    let out = command(
        bin,
        &home,
        &ws,
        &["--no-daemon", "sessions", "tree", "missing"],
    )
    .output()?;
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("session not found: missing"));
    Ok(())
}

fn ingest_start(bin: &str, home: &Path, ws: &Path, id: &str) -> anyhow::Result<()> {
    let payload =
        format!(r#"{{"event":"SessionStart","session_id":"{id}","timestamp_ms":1714000000000}}"#);
    let mut child = command(
        bin,
        home,
        ws,
        &[
            "--no-daemon",
            "ingest",
            "hook",
            "--source",
            "cursor",
            "--workspace",
            path(ws),
        ],
    )
    .stdin(Stdio::piped())
    .spawn()?;
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(payload.as_bytes())?;
    let out = child.wait_with_output()?;
    anyhow::ensure!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    Ok(())
}

fn run(bin: &str, home: &Path, cwd: &Path, args: &[&str]) -> anyhow::Result<()> {
    let out = command(bin, home, cwd, args).output()?;
    anyhow::ensure!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    Ok(())
}

fn output(bin: &str, home: &Path, cwd: &Path, args: &[&str]) -> anyhow::Result<String> {
    let out = command(bin, home, cwd, args).output()?;
    anyhow::ensure!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    Ok(String::from_utf8(out.stdout)?)
}

fn command<'a>(bin: &'a str, home: &Path, cwd: &Path, args: &[&str]) -> Command {
    let mut cmd = Command::new(bin);
    cmd.current_dir(cwd)
        .env("HOME", home)
        .env("KAIZEN_HOME", home.join(".kaizen"))
        .args(args);
    cmd
}

fn path(path: &Path) -> &str {
    path.to_str().unwrap()
}
