// SPDX-License-Identifier: AGPL-3.0-or-later
//! Local daemon client/lifecycle API.

mod server;
mod worker;

use crate::core::paths::kaizen_dir;
use crate::ipc::{
    ClientHello, ClientKind, DaemonRequest, DaemonResponse, DaemonStatus, PROTO_VERSION,
    ServerHello, read_frame, write_frame,
};
use anyhow::{Context, Result, anyhow};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use tokio::net::UnixStream;

const PID_FILE: &str = "daemon.pid";
const SOCK_FILE: &str = "daemon.sock";
const LOG_FILE: &str = "daemon.log";
const START_WAIT_MS: u64 = 2_000;

#[derive(Debug, Clone)]
pub struct RuntimePaths {
    pub dir: PathBuf,
    pub pid: PathBuf,
    pub sock: PathBuf,
    pub log: PathBuf,
}

pub fn enabled() -> bool {
    if let Ok(v) = std::env::var("KAIZEN_DAEMON") {
        return v != "0";
    }
    std::env::args()
        .next()
        .and_then(|p| PathBuf::from(p).file_stem().map(|s| s.to_owned()))
        .and_then(|s| s.to_str().map(str::to_string))
        .is_some_and(|name| name == "kaizen")
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
    let paths = runtime_paths()?;
    std::fs::create_dir_all(&paths.dir)?;
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&paths.log)
        .with_context(|| format!("open daemon log: {}", paths.log.display()))?;
    let err = log.try_clone()?;
    Command::new(std::env::current_exe()?)
        .args(["daemon", "start", "--background"])
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(err))
        .spawn()
        .context("spawn kaizen daemon")?;
    let deadline = Instant::now() + Duration::from_millis(START_WAIT_MS);
    while Instant::now() < deadline {
        if try_status().is_ok() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    Err(anyhow!(
        "daemon did not become ready at {}",
        paths.sock.display()
    ))
}

pub fn request_blocking(request: DaemonRequest) -> Result<DaemonResponse> {
    ensure_running()?;
    tokio::runtime::Runtime::new()?.block_on(request_async(request))
}

pub fn try_status() -> Result<DaemonStatus> {
    let response =
        tokio::runtime::Runtime::new()?.block_on(request_async(DaemonRequest::Status))?;
    match response {
        DaemonResponse::Status(status) => Ok(status),
        DaemonResponse::Error { message, .. } => Err(anyhow!(message)),
        _ => Err(anyhow!("unexpected daemon status response")),
    }
}

pub fn start_foreground() -> Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(server::run_server())
}

pub fn stop() -> Result<String> {
    match tokio::runtime::Runtime::new()?.block_on(request_async(DaemonRequest::Stop))? {
        DaemonResponse::Ack { message } => Ok(message),
        DaemonResponse::Error { message, .. } => Err(anyhow!(message)),
        _ => Err(anyhow!("unexpected daemon stop response")),
    }
}

pub fn hello_blocking(client: ClientKind, workspace: Option<String>) -> Result<ServerHello> {
    match request_blocking(DaemonRequest::Hello(ClientHello {
        proto_version: PROTO_VERSION,
        client,
        workspace,
    }))? {
        DaemonResponse::Hello(hello) => Ok(hello),
        DaemonResponse::Error { message, .. } => Err(anyhow!(message)),
        _ => Err(anyhow!("unexpected daemon hello response")),
    }
}

async fn request_async(request: DaemonRequest) -> Result<DaemonResponse> {
    let paths = runtime_paths()?;
    let mut stream = UnixStream::connect(&paths.sock)
        .await
        .with_context(|| format!("connect daemon socket: {}", paths.sock.display()))?;
    write_frame(&mut stream, &request).await?;
    read_frame(&mut stream).await
}
