// SPDX-License-Identifier: AGPL-3.0-or-later
//! Background daemon process startup.

use super::lifecycle::{
    DaemonStatusOutcome, RuntimePaths, runtime_paths, runtime_paths_for, status_outcome,
};
use crate::ipc::{DaemonStatus, WebEndpoint};
use anyhow::{Context, Result, anyhow};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

// Restore runs migrations, backfills, first scans before readiness. Keep slow startup bounded.
const START_WAIT_MS: u64 = 30_000;
const STOP_WAIT_MS: u64 = 2_000;

#[derive(Debug, Clone)]
pub struct BackgroundStart {
    pub pid: u32,
    pub paths: RuntimePaths,
    pub already_running: bool,
    pub web: Option<WebEndpoint>,
}

pub fn start_background() -> Result<BackgroundStart> {
    start_background_at(runtime_paths()?)
}

pub fn start_background_for(workspace: &Path) -> Result<BackgroundStart> {
    start_background_at(runtime_paths_for(workspace)?)
}

pub fn restart_background() -> Result<BackgroundStart> {
    if !daemon_stopped()? {
        super::stop()?;
        wait_until_stopped()?;
    }
    start_background()
}

fn wait_until_stopped() -> Result<()> {
    let deadline = Instant::now() + Duration::from_millis(STOP_WAIT_MS);
    while Instant::now() < deadline {
        if daemon_stopped()? {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    Err(anyhow!("daemon did not stop within {STOP_WAIT_MS}ms"))
}

fn daemon_stopped() -> Result<bool> {
    Ok(matches!(
        super::status_outcome()?,
        super::DaemonStatusOutcome::Stopped { .. }
    ))
}

fn start_background_at(paths: RuntimePaths) -> Result<BackgroundStart> {
    if let Some(start) = status_start(&paths, true)? {
        return Ok(start);
    }
    std::fs::create_dir_all(&paths.dir)?;
    let mut child = spawn_background(&paths)?;
    wait_until_ready(paths, &mut child)
}

fn status_start(paths: &RuntimePaths, already_running: bool) -> Result<Option<BackgroundStart>> {
    match status_outcome()? {
        DaemonStatusOutcome::Running(status) => Ok(Some(background_start(
            status,
            paths.clone(),
            already_running,
        ))),
        DaemonStatusOutcome::Stopped { .. } => Ok(None),
    }
}

fn spawn_background(paths: &RuntimePaths) -> Result<Child> {
    let log = open_log(&paths.log)?;
    let err = log.try_clone()?;
    background_command(log, err)?
        .spawn()
        .context("spawn kaizen daemon")
}

fn open_log(path: &Path) -> Result<std::fs::File> {
    crate::core::safe_fs::append(path)
        .with_context(|| format!("open daemon log: {}", path.display()))
}

fn wait_until_ready(paths: RuntimePaths, child: &mut Child) -> Result<BackgroundStart> {
    let deadline = Instant::now() + Duration::from_millis(START_WAIT_MS);
    while Instant::now() < deadline {
        if let Some(start) = poll_start(child, &paths)? {
            return Ok(start);
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    Err(start_timeout(&paths))
}

fn poll_start(child: &mut Child, paths: &RuntimePaths) -> Result<Option<BackgroundStart>> {
    if let Some(status) = child.try_wait().context("poll daemon child")? {
        return Err(early_exit(status, paths));
    }
    status_start(paths, false)
}

fn background_start(
    status: DaemonStatus,
    paths: RuntimePaths,
    already_running: bool,
) -> BackgroundStart {
    BackgroundStart {
        pid: status.pid,
        paths,
        already_running,
        web: status.web,
    }
}

fn early_exit(status: std::process::ExitStatus, paths: &RuntimePaths) -> anyhow::Error {
    anyhow!(
        "daemon exited before ready with status {status}; see {}",
        paths.log.display()
    )
}

fn start_timeout(paths: &RuntimePaths) -> anyhow::Error {
    anyhow!(
        "daemon did not become ready at {}; see {}",
        paths.sock.display(),
        paths.log.display()
    )
}

fn background_command(log: std::fs::File, err: std::fs::File) -> Result<Command> {
    let mut command = Command::new(std::env::current_exe()?);
    command
        .args(["daemon", "start"])
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(err));
    detach_background(&mut command);
    Ok(command)
}

#[cfg(unix)]
fn detach_background(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    unsafe {
        command.pre_exec(|| {
            (libc::setsid() != -1)
                .then_some(())
                .ok_or_else(std::io::Error::last_os_error)
        });
    }
}

#[cfg(not(unix))]
fn detach_background(_command: &mut Command) {}
