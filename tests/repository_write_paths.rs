// SPDX-License-Identifier: AGPL-3.0-or-later
//! Reject project-data file symlinks that target the observed repository.

#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_kaizen")
}

fn run(home: &Path, workspace: &Path, args: &[&str]) -> Output {
    run_with_home(home, workspace, &home.join(".kaizen"), args)
}

fn run_with_home(home: &Path, workspace: &Path, kaizen_home: &Path, args: &[&str]) -> Output {
    Command::new(bin())
        .current_dir(workspace)
        .env("HOME", home)
        .env("KAIZEN_HOME", kaizen_home)
        .arg("--no-daemon")
        .args(args)
        .output()
        .unwrap()
}

fn project_data(home: &Path, workspace: &Path) -> PathBuf {
    let slug = kaizen::core::paths::workspace_slug(&workspace.canonicalize().unwrap());
    home.join(".kaizen/projects").join(slug)
}

fn with_kaizen_home<T>(home: &Path, run: impl FnOnce() -> T) -> T {
    let previous = std::env::var_os("KAIZEN_HOME");
    unsafe { std::env::set_var("KAIZEN_HOME", home.join(".kaizen")) };
    let result = run();
    match previous {
        Some(value) => unsafe { std::env::set_var("KAIZEN_HOME", value) },
        None => unsafe { std::env::remove_var("KAIZEN_HOME") },
    }
    result
}

fn setup() -> anyhow::Result<(tempfile::TempDir, PathBuf, PathBuf)> {
    let tmp = tempfile::tempdir()?;
    let home = tmp.path().join("home");
    let workspace = tmp.path().join("repo");
    std::fs::create_dir_all(&workspace)?;
    std::fs::write(workspace.join("README.md"), "target repository\n")?;
    std::fs::create_dir_all(project_data(&home, &workspace))?;
    Ok((tmp, home, workspace))
}

#[test]
fn init_rejects_config_symlinked_to_workspace() -> anyhow::Result<()> {
    let (_tmp, home, workspace) = setup()?;
    let target = workspace.join("config.toml");
    std::os::unix::fs::symlink(&target, project_data(&home, &workspace).join("config.toml"))?;

    let output = run(&home, &workspace, &["init"]);

    assert!(!output.status.success());
    assert!(!target.exists());
    Ok(())
}

#[test]
fn telemetry_rejects_file_symlinked_to_workspace() -> anyhow::Result<()> {
    let (_tmp, home, workspace) = setup()?;
    std::fs::create_dir_all(home.join(".kaizen"))?;
    std::fs::write(
        home.join(".kaizen/config.toml"),
        "[[telemetry.exporters]]\ntype = \"file\"\n",
    )?;
    let target = workspace.join("telemetry.ndjson");
    let link = project_data(&home, &workspace).join("telemetry.ndjson");
    std::os::unix::fs::symlink(&target, link)?;

    let output = run(&home, &workspace, &["telemetry", "test"]);

    assert!(!output.status.success());
    assert!(!target.exists());
    Ok(())
}

#[test]
fn telemetry_rejects_file_hardlinked_to_workspace() -> anyhow::Result<()> {
    let (_tmp, home, workspace) = setup()?;
    std::fs::create_dir_all(home.join(".kaizen"))?;
    std::fs::write(
        home.join(".kaizen/config.toml"),
        "[[telemetry.exporters]]\ntype = \"file\"\n",
    )?;
    let target = workspace.join("telemetry.ndjson");
    std::fs::write(&target, "repository bytes\n")?;
    let link = project_data(&home, &workspace).join("telemetry.ndjson");
    std::fs::hard_link(&target, link)?;

    let output = run(&home, &workspace, &["telemetry", "test"]);

    assert!(!output.status.success());
    assert_eq!(std::fs::read_to_string(target)?, "repository bytes\n");
    Ok(())
}

#[test]
fn database_rejects_file_hardlinked_to_workspace() -> anyhow::Result<()> {
    let (_tmp, home, workspace) = setup()?;
    let target = workspace.join("kaizen.db");
    std::fs::write(&target, "repository bytes\n")?;
    std::fs::hard_link(&target, project_data(&home, &workspace).join("kaizen.db"))?;
    let result = with_kaizen_home(&home, || kaizen::core::workspace::db_path(&workspace));

    assert!(result.is_err());
    assert_eq!(std::fs::read_to_string(target)?, "repository bytes\n");
    Ok(())
}

#[test]
fn telemetry_configure_rejects_home_inside_workspace() -> anyhow::Result<()> {
    let (_tmp, home, workspace) = setup()?;
    let kaizen_home = workspace.join(".kaizen-home");
    let output = run_with_home(
        &home,
        &workspace,
        &kaizen_home,
        &[
            "telemetry",
            "configure",
            "--type",
            "file",
            "--non-interactive",
        ],
    );
    assert!(!output.status.success());
    assert!(!kaizen_home.exists());
    Ok(())
}

#[test]
fn telemetry_configure_rejects_hardlinked_config() -> anyhow::Result<()> {
    let (_tmp, home, workspace) = setup()?;
    let target = workspace.join("global-config.toml");
    std::fs::write(&target, "repository bytes\n")?;
    std::fs::hard_link(&target, home.join(".kaizen/config.toml"))?;
    let output = run(
        &home,
        &workspace,
        &[
            "telemetry",
            "configure",
            "--type",
            "file",
            "--non-interactive",
        ],
    );
    assert!(!output.status.success());
    assert_eq!(std::fs::read_to_string(target)?, "repository bytes\n");
    Ok(())
}

#[test]
fn local_salt_rejects_hardlink() -> anyhow::Result<()> {
    let (_tmp, home, workspace) = setup()?;
    let target = workspace.join("salt.txt");
    std::fs::write(&target, "repository bytes\n")?;
    std::fs::hard_link(&target, home.join(".kaizen/local_salt.hex"))?;
    let sync = kaizen::core::config::SyncConfig::default();
    let result = kaizen::core::config::effective_redaction_salt(&sync, &home.join(".kaizen"));
    assert!(result.is_err());
    assert_eq!(std::fs::read_to_string(target)?, "repository bytes\n");
    Ok(())
}

#[test]
fn machine_registry_rejects_hardlinked_database() -> anyhow::Result<()> {
    let (_tmp, home, workspace) = setup()?;
    let target = workspace.join("machine.db");
    std::fs::write(&target, [])?;
    std::fs::hard_link(&target, home.join(".kaizen/machine.db"))?;
    let _ = run(&home, &workspace, &["init"]);
    assert_eq!(std::fs::read(target)?, Vec::<u8>::new());
    Ok(())
}
