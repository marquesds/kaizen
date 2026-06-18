// SPDX-License-Identifier: AGPL-3.0-or-later
//! `kaizen doctor` validates workspace before creating persistent state.

use std::path::Path;
use std::process::{Command, Output};

fn run_doctor(home: &Path, workspace: &Path) -> anyhow::Result<Output> {
    Ok(Command::new(env!("CARGO_BIN_EXE_kaizen"))
        .args(["doctor", "--workspace"])
        .arg(workspace)
        .env("KAIZEN_HOME", home)
        .output()?)
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn assert_healthy_output(text: &str) {
    assert!(text.contains("kaizen") && text.contains("doctor"), "{text}");
    assert!(
        text.contains("store: OK") || text.contains("sessions in store"),
        "{text}"
    );
    assert!(text.contains("hooks:"), "{text}");
    assert!(text.contains("machine registry:"), "{text}");
}

fn assert_no_workspace_state(home: &Path) {
    assert!(!home.join("machine.db").exists(), "registry was created");
    assert!(
        !home.join("projects").exists(),
        "project data/store was created"
    );
}

#[test]
fn doctor_runs_in_temp_workspace() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let workspace = tmp.path().join("workspace");
    std::fs::create_dir(&workspace)?;
    let output = run_doctor(&tmp.path().join("home"), &workspace)?;
    let text = stdout(&output);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_healthy_output(&text);
    Ok(())
}

#[test]
fn doctor_rejects_missing_workspace_without_state() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let home = tmp.path().join("home");
    let output = run_doctor(&home, &tmp.path().join("missing"))?;
    let error = stderr(&output);
    assert!(!output.status.success(), "doctor unexpectedly succeeded");
    assert!(error.contains("workspace does not exist"), "{error}");
    assert_no_workspace_state(&home);
    Ok(())
}

fn run(home: &Path, workspace: &Path, args: &[&str]) -> anyhow::Result<Output> {
    Ok(Command::new(env!("CARGO_BIN_EXE_kaizen"))
        .args(args)
        .current_dir(workspace)
        .env("HOME", home)
        .env("KAIZEN_HOME", home.join(".kaizen"))
        .output()?)
}

fn sessions_args(workspace: &Path, direct: bool) -> Vec<String> {
    let mut args = direct
        .then_some("--no-daemon")
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    args.extend(["sessions", "list", "--workspace"].map(str::to_string));
    args.push(workspace.to_string_lossy().into_owned());
    args.push("--json".into());
    args
}

fn run_sessions(home: &Path, workspace: &Path, direct: bool) -> anyhow::Result<Output> {
    let args = sessions_args(workspace, direct);
    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    run(home, workspace, &refs)
}

#[test]
fn direct_sessions_read_does_not_create_workspace_state() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let home = tmp.path().join("home");
    let workspace = tmp.path().join("workspace");
    std::fs::create_dir_all(&workspace)?;
    let output = run_sessions(&home, &workspace, true)?;
    assert!(output.status.success(), "{}", stderr(&output));
    assert_no_workspace_state(&home.join(".kaizen"));
    Ok(())
}

#[test]
fn daemon_sessions_read_does_not_create_workspace_state() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let home = tmp.path().join("home");
    let workspace = tmp.path().join("workspace");
    std::fs::create_dir_all(&workspace)?;
    let start = run(&home, &workspace, &["daemon", "start", "--background"])?;
    let output = run_sessions(&home, &workspace, false)?;
    let _ = run(&home, &workspace, &["daemon", "stop"]);
    assert!(start.status.success(), "{}", stderr(&start));
    assert!(output.status.success(), "{}", stderr(&output));
    assert_no_workspace_state(&home.join(".kaizen"));
    Ok(())
}

#[test]
fn empty_reports_do_not_create_workspace_state() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let workspace = tmp.path().join("workspace");
    std::fs::create_dir_all(&workspace)?;
    let cases: &[(&str, &[&str])] = &[
        ("summary", &["--no-daemon", "summary", "--json"]),
        ("metrics", &["--no-daemon", "metrics", "--json"]),
        ("insights", &["--no-daemon", "insights"]),
    ];
    for (name, args) in cases {
        let home = tmp.path().join(name);
        let output = run(&home, &workspace, args)?;
        assert!(output.status.success(), "{name}: {}", stderr(&output));
        assert_no_workspace_state(&home.join(".kaizen"));
    }
    Ok(())
}
