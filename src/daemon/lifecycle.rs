// SPDX-License-Identifier: AGPL-3.0-or-later
//! Daemon process lifecycle.

use crate::ipc::{DaemonRequest, DaemonResponse, DaemonStatus};
use anyhow::{Result, anyhow};
use std::path::{Path, PathBuf};

const PID_FILE: &str = "daemon.pid";
const SOCK_FILE: &str = "daemon.sock";
const LOG_FILE: &str = "daemon.log";
const TOKEN_FILE: &str = "web_token.hex";

#[derive(Debug, Clone)]
pub struct RuntimePaths {
    pub dir: PathBuf,
    pub pid: PathBuf,
    pub sock: PathBuf,
    pub log: PathBuf,
    pub token: PathBuf,
}

#[derive(Debug, Clone)]
pub enum DaemonStatusOutcome {
    Running(DaemonStatus),
    Stopped { socket: PathBuf },
}

pub fn enabled() -> bool {
    if let Ok(value) = std::env::var("KAIZEN_DAEMON") {
        return value != "0";
    }
    executable_name().is_some_and(|name| name == "kaizen")
}

fn executable_name() -> Option<String> {
    std::env::args()
        .next()
        .and_then(|path| PathBuf::from(path).file_stem().map(|stem| stem.to_owned()))
        .and_then(|stem| stem.to_str().map(str::to_string))
}

pub fn runtime_paths() -> Result<RuntimePaths> {
    runtime_paths_for(&std::env::current_dir()?)
}

pub fn runtime_paths_for(workspace: &Path) -> Result<RuntimePaths> {
    let dir = crate::core::home_paths::root(workspace)?;
    let child = |name| crate::core::paths::descendant_path(&dir, Path::new(name));
    Ok(RuntimePaths {
        pid: child(PID_FILE)?,
        sock: child(SOCK_FILE)?,
        log: child(LOG_FILE)?,
        token: child(TOKEN_FILE)?,
        dir,
    })
}

pub fn ensure_running() -> Result<()> {
    ensure_running_for(&std::env::current_dir()?)
}

pub fn ensure_running_for(workspace: &Path) -> Result<()> {
    runtime_paths_for(workspace)?;
    if !enabled() || try_status().is_ok() {
        return Ok(());
    }
    super::start_background_for(workspace).map(|_| ())
}

pub fn try_status() -> Result<DaemonStatus> {
    let response = tokio::runtime::Runtime::new()?
        .block_on(super::client::request_async(DaemonRequest::Status))?;
    match response {
        DaemonResponse::Status(status) => Ok(status),
        DaemonResponse::Error { message, .. } => Err(anyhow!(message)),
        _ => Err(anyhow!("unexpected daemon status response")),
    }
}

pub fn status_outcome() -> Result<DaemonStatusOutcome> {
    match try_status() {
        Ok(status) => Ok(DaemonStatusOutcome::Running(status)),
        Err(err) if is_daemon_unavailable(&err) => Ok(DaemonStatusOutcome::Stopped {
            socket: runtime_paths()?.sock,
        }),
        Err(err) => Err(err),
    }
}

fn is_daemon_unavailable(err: &anyhow::Error) -> bool {
    err.chain().any(is_unavailable_cause)
}

fn is_unavailable_cause(cause: &(dyn std::error::Error + 'static)) -> bool {
    if cause.to_string().starts_with("connect daemon socket:") {
        return true;
    }
    cause.downcast_ref::<std::io::Error>().is_some_and(|error| {
        matches!(
            error.kind(),
            std::io::ErrorKind::UnexpectedEof
                | std::io::ErrorKind::ConnectionReset
                | std::io::ErrorKind::BrokenPipe
        )
    })
}

pub fn start_foreground() -> Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(super::server::run_server())
}

pub fn stop() -> Result<String> {
    let response = tokio::runtime::Runtime::new()?
        .block_on(super::client::request_async(DaemonRequest::Stop))?;
    match response {
        DaemonResponse::Ack { message } => Ok(message),
        DaemonResponse::Error { message, .. } => Err(anyhow!(message)),
        _ => Err(anyhow!("unexpected daemon stop response")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eof_after_shutdown_is_unavailable() {
        let error = anyhow::Error::new(std::io::Error::from(std::io::ErrorKind::UnexpectedEof));
        assert!(is_daemon_unavailable(&error));
    }
}
