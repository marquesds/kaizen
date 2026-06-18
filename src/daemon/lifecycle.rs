// SPDX-License-Identifier: AGPL-3.0-or-later
//! Daemon process lifecycle.

use crate::core::paths::kaizen_dir;
use crate::ipc::{DaemonRequest, DaemonResponse, DaemonStatus};
use anyhow::{Result, anyhow};
use std::path::PathBuf;

const PID_FILE: &str = "daemon.pid";
const SOCK_FILE: &str = "daemon.sock";
const LOG_FILE: &str = "daemon.log";

#[derive(Debug, Clone)]
pub struct RuntimePaths {
    pub dir: PathBuf,
    pub pid: PathBuf,
    pub sock: PathBuf,
    pub log: PathBuf,
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
    let dir = kaizen_dir().ok_or_else(|| anyhow!("KAIZEN_HOME/HOME not set"))?;
    Ok(RuntimePaths {
        pid: dir.join(PID_FILE),
        sock: dir.join(SOCK_FILE),
        log: dir.join(LOG_FILE),
        dir,
    })
}

pub fn ensure_running() -> Result<()> {
    if !enabled() || try_status().is_ok() {
        return Ok(());
    }
    super::start_background().map(|_| ())
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
        Err(err) if is_socket_connect_error(&err) => Ok(DaemonStatusOutcome::Stopped {
            socket: runtime_paths()?.sock,
        }),
        Err(err) => Err(err),
    }
}

fn is_socket_connect_error(err: &anyhow::Error) -> bool {
    err.chain()
        .any(|cause| cause.to_string().starts_with("connect daemon socket:"))
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
