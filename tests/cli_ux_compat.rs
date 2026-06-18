// SPDX-License-Identifier: AGPL-3.0-or-later
//! Compatibility coverage for root search and projects UX.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

struct Fixture {
    _tmp: tempfile::TempDir,
    home: PathBuf,
    live: PathBuf,
    missing: PathBuf,
}

impl Fixture {
    fn new() -> anyhow::Result<Self> {
        let tmp = tempfile::tempdir()?;
        let (home, live, missing) = fixture_paths(tmp.path());
        create_dirs([&home, &live, &missing])?;
        Ok(Self {
            _tmp: tmp,
            home,
            live,
            missing,
        })
    }

    fn init(&self, workspace: &Path) -> Output {
        run(&self.home, workspace, &["init"])
    }

    fn run_live(&self, args: &[&str]) -> Output {
        run(&self.home, &self.live, args)
    }
}

fn fixture_paths(root: &Path) -> (PathBuf, PathBuf, PathBuf) {
    (root.join("home"), root.join("live"), root.join("missing"))
}

fn create_dirs(paths: [&Path; 3]) -> anyhow::Result<()> {
    Ok(paths.into_iter().try_for_each(std::fs::create_dir_all)?)
}

fn run(home: &Path, cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_kaizen"))
        .current_dir(cwd)
        .env("HOME", home)
        .env("KAIZEN_HOME", home.join(".kaizen"))
        .arg("--no-daemon")
        .args(args)
        .output()
        .unwrap()
}

fn success(output: Output) -> String {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn project_rows(fixture: &Fixture, args: &[&str]) -> Value {
    let text = success(run(&fixture.home, &fixture.live, args));
    serde_json::from_str(&text).unwrap()
}

#[test]
fn search_root_and_sessions_alias_share_query_behavior() -> anyhow::Result<()> {
    let fixture = Fixture::new()?;
    success(fixture.init(&fixture.live));
    success(fixture.run_live(&["search", "reindex"]));
    success(fixture.run_live(&["search", "needle", "--limit", "3"]));
    success(fixture.run_live(&["sessions", "search", "needle"]));
    let literal = success(fixture.run_live(&["search", "--", "reindex"]));
    assert!(literal.contains("SESSION"));
    Ok(())
}

#[test]
fn projects_default_hides_missing_and_alias_can_show_status() -> anyhow::Result<()> {
    let fixture = Fixture::new()?;
    success(fixture.init(&fixture.live));
    success(fixture.init(&fixture.missing));
    std::fs::remove_dir_all(&fixture.missing)?;
    assert_project_rows(&fixture);
    Ok(())
}

fn assert_project_rows(fixture: &Fixture) {
    let current = project_rows(fixture, &["projects", "--json"]);
    let all = project_rows(
        fixture,
        &["projects", "list", "--include-missing", "--json"],
    );
    assert_eq!(current.as_array().unwrap().len(), 1);
    assert_eq!(all.as_array().unwrap().len(), 2);
    assert!(all.as_array().unwrap().iter().any(is_missing));
}

fn is_missing(row: &Value) -> bool {
    row["status"] == "missing"
}

#[test]
fn missing_project_cannot_be_resolved() -> anyhow::Result<()> {
    let fixture = Fixture::new()?;
    success(fixture.init(&fixture.missing));
    std::fs::remove_dir_all(&fixture.missing)?;
    let output = fixture.run_live(&["doctor", "--project", "missing"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown project"));
    Ok(())
}

#[test]
fn metrics_report_subcommand_matches_root_report() -> anyhow::Result<()> {
    let fixture = Fixture::new()?;
    success(fixture.init(&fixture.live));
    success(fixture.run_live(&["metrics", "--json"]));
    success(fixture.run_live(&["metrics", "report", "--json"]));
    Ok(())
}
