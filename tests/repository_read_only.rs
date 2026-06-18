// SPDX-License-Identifier: AGPL-3.0-or-later
//! Regression coverage for Kaizen's read-only workspace contract.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::{Mutex, OnceLock};

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_kaizen")
}

fn run(home: &Path, workspace: &Path, args: &[&str]) -> Output {
    run_with_kaizen_home(home, workspace, &home.join(".kaizen"), args)
}

fn run_with_kaizen_home(
    home: &Path,
    workspace: &Path,
    kaizen_home: &Path,
    args: &[&str],
) -> Output {
    Command::new(bin())
        .current_dir(workspace)
        .env("HOME", home)
        .env("KAIZEN_HOME", kaizen_home)
        .arg("--no-daemon")
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn init_rejects_kaizen_home_inside_workspace_without_mutation() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let home = tmp.path().join("home");
    let workspace = tmp.path().join("repo");
    std::fs::create_dir_all(&workspace)?;
    std::fs::write(workspace.join("README.md"), "target repository\n")?;
    let before = snapshot(&workspace)?;

    let output = run_with_kaizen_home(
        &home,
        &workspace,
        &workspace.join(".kaizen-home"),
        &["init"],
    );

    assert!(!output.status.success());
    assert_eq!(snapshot(&workspace)?, before);
    Ok(())
}

#[test]
fn init_rejects_relative_kaizen_home_inside_workspace() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let home = tmp.path().join("home");
    let workspace = tmp.path().join("repo");
    std::fs::create_dir_all(&workspace)?;
    std::fs::write(workspace.join("README.md"), "target repository\n")?;
    let before = snapshot(&workspace)?;

    let output = run_with_kaizen_home(&home, &workspace, Path::new(".kaizen-home"), &["init"]);

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("KAIZEN_HOME"));
    assert_eq!(snapshot(&workspace)?, before);
    Ok(())
}

#[cfg(unix)]
#[test]
fn init_rejects_project_data_symlinked_to_workspace() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let home = tmp.path().join("home");
    let workspace = tmp.path().join("repo");
    std::fs::create_dir_all(&workspace)?;
    std::fs::write(workspace.join("README.md"), "target repository\n")?;
    let data = project_data(&home, &workspace);
    std::fs::create_dir_all(data.parent().unwrap())?;
    std::os::unix::fs::symlink(&workspace, &data)?;
    let before = snapshot(&workspace)?;

    let output = run(&home, &workspace, &["init"]);

    assert!(!output.status.success());
    assert_eq!(snapshot(&workspace)?, before);
    Ok(())
}

#[test]
fn metrics_index_never_falls_back_to_workspace() -> anyhow::Result<()> {
    let _guard = env_lock().lock().unwrap();
    let tmp = tempfile::tempdir()?;
    let workspace = tmp.path().join("repo");
    std::fs::create_dir_all(&workspace)?;
    std::fs::write(workspace.join("main.rs"), "fn main() {}\n")?;
    let store = kaizen::store::Store::open(&tmp.path().join("store.db"))?;
    let previous = std::env::var_os("KAIZEN_HOME");
    unsafe { std::env::set_var("KAIZEN_HOME", workspace.join(".kaizen-home")) };
    let result = kaizen::metrics::index::ensure_indexed(&store, &workspace, true);
    restore_env("KAIZEN_HOME", previous);

    assert!(result.is_err());
    assert!(!workspace.join("codegraph.db").exists());
    Ok(())
}

fn restore_env(key: &str, value: Option<std::ffi::OsString>) {
    match value {
        Some(value) => unsafe { std::env::set_var(key, value) },
        None => unsafe { std::env::remove_var(key) },
    }
}

fn snapshot(root: &Path) -> anyhow::Result<BTreeMap<PathBuf, Vec<u8>>> {
    let mut files = BTreeMap::new();
    collect_files(root, root, &mut files)?;
    Ok(files)
}

fn collect_files(
    root: &Path,
    dir: &Path,
    files: &mut BTreeMap<PathBuf, Vec<u8>>,
) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_files(root, &path, files)?;
        } else {
            files.insert(path.strip_prefix(root)?.to_path_buf(), std::fs::read(path)?);
        }
    }
    Ok(())
}

fn project_data(home: &Path, workspace: &Path) -> PathBuf {
    let workspace = workspace.canonicalize().unwrap();
    let slug = kaizen::core::paths::workspace_slug(&workspace);
    home.join(".kaizen/projects").join(slug)
}

#[test]
fn init_copies_legacy_data_without_mutating_workspace() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let home = tmp.path().join("home");
    let workspace = tmp.path().join("repo");
    std::fs::create_dir_all(workspace.join(".kaizen/search"))?;
    std::fs::write(workspace.join("README.md"), "target repository\n")?;
    std::fs::write(workspace.join(".kaizen/config.toml"), "[capture]\n")?;
    std::fs::write(workspace.join(".kaizen/search/index"), b"legacy")?;
    let before = snapshot(&workspace)?;

    for _ in 0..2 {
        let output = run(&home, &workspace, &["init"]);
        anyhow::ensure!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(snapshot(&workspace)?, before);
    }

    let data = project_data(&home, &workspace);
    assert_eq!(std::fs::read(data.join("search/index"))?, b"legacy");
    assert!(data.join("LEGACY_IMPORTED.txt").exists());
    assert!(home.join(".claude/settings.json").exists());
    assert!(home.join(".cursor/hooks.json").exists());
    Ok(())
}

#[test]
fn guidance_apply_is_rejected_without_mutating_workspace() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let home = tmp.path().join("home");
    let workspace = tmp.path().join("repo");
    let rule = workspace.join(".cursor/rules/dead.mdc");
    std::fs::create_dir_all(rule.parent().unwrap())?;
    std::fs::write(&rule, "rule text\n")?;

    let output = run(
        &home,
        &workspace,
        &["guidance", "propose", "--artifact", "rule:dead", "--apply"],
    );

    assert!(!output.status.success());
    assert_eq!(std::fs::read_to_string(rule)?, "rule text\n");
    Ok(())
}
