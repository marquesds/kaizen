// SPDX-License-Identifier: AGPL-3.0-or-later
//! Daemon runtime state must never resolve into the observed repository.

#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn with_home<T>(home: &Path, run: impl FnOnce() -> T) -> T {
    let _guard = env_lock().lock().unwrap();
    let previous = std::env::var_os("KAIZEN_HOME");
    unsafe { std::env::set_var("KAIZEN_HOME", home) };
    let result = run();
    match previous {
        Some(value) => unsafe { std::env::set_var("KAIZEN_HOME", value) },
        None => unsafe { std::env::remove_var("KAIZEN_HOME") },
    }
    result
}

fn setup() -> anyhow::Result<(tempfile::TempDir, PathBuf, PathBuf)> {
    let tmp = tempfile::tempdir()?;
    let home = tmp.path().join("home/.kaizen");
    let workspace = tmp.path().join("repo");
    std::fs::create_dir_all(&home)?;
    std::fs::create_dir_all(&workspace)?;
    Ok((tmp, home, workspace))
}

#[test]
fn daemon_rejects_home_inside_workspace() -> anyhow::Result<()> {
    let (_tmp, _home, workspace) = setup()?;
    let inside = workspace.join(".kaizen-home");
    let result = with_home(&inside, || kaizen::daemon::ensure_running_for(&workspace));
    assert!(result.is_err());
    assert!(!inside.exists());
    Ok(())
}

#[test]
fn daemon_rejects_hardlinked_runtime_file() -> anyhow::Result<()> {
    let (_tmp, home, workspace) = setup()?;
    let target = workspace.join("daemon.log");
    std::fs::write(&target, "repository bytes\n")?;
    std::fs::hard_link(&target, home.join("daemon.log"))?;
    let result = with_home(&home, || kaizen::daemon::ensure_running_for(&workspace));
    assert!(result.is_err());
    assert_eq!(std::fs::read_to_string(target)?, "repository bytes\n");
    Ok(())
}
