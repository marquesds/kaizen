// SPDX-License-Identifier: AGPL-3.0-or-later
//! Local daemon client/lifecycle API.

mod capture_status;
mod proxy_task;
mod scanner_task;
mod server;
mod supervisor;
mod worker;

use crate::core::paths::kaizen_dir;
use crate::ipc::{
    CaptureStatus, ClientHello, ClientKind, DaemonRequest, DaemonResponse, DaemonStatus,
    ObservedSession, PROTO_VERSION, ProxyEndpoint, ServerHello, read_frame, write_frame,
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

#[derive(Debug, Clone)]
pub struct BackgroundStart {
    pub pid: u32,
    pub paths: RuntimePaths,
    pub already_running: bool,
}

#[derive(Debug, Clone)]
pub enum DaemonStatusOutcome {
    Running(DaemonStatus),
    Stopped { socket: PathBuf },
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
    start_background().map(|_| ())
}

pub fn start_background() -> Result<BackgroundStart> {
    let paths = runtime_paths()?;
    if let Ok(status) = try_status() {
        return Ok(BackgroundStart {
            pid: status.pid,
            paths,
            already_running: true,
        });
    }
    std::fs::create_dir_all(&paths.dir)?;
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&paths.log)
        .with_context(|| format!("open daemon log: {}", paths.log.display()))?;
    let err = log.try_clone()?;
    let mut child = Command::new(std::env::current_exe()?)
        .args(["daemon", "start"])
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(err))
        .spawn()
        .context("spawn kaizen daemon")?;
    let deadline = Instant::now() + Duration::from_millis(START_WAIT_MS);
    while Instant::now() < deadline {
        if let Some(status) = child.try_wait().context("poll daemon child")? {
            return Err(anyhow!(
                "daemon exited before ready with status {status}; see {}",
                paths.log.display()
            ));
        }
        if let Ok(status) = try_status() {
            return Ok(BackgroundStart {
                pid: status.pid,
                paths,
                already_running: false,
            });
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    Err(anyhow!(
        "daemon did not become ready at {}; see {}",
        paths.sock.display(),
        paths.log.display()
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

pub fn status_outcome() -> Result<DaemonStatusOutcome> {
    match try_status() {
        Ok(status) => Ok(DaemonStatusOutcome::Running(status)),
        Err(err) if is_daemon_socket_connect_error(&err) => Ok(DaemonStatusOutcome::Stopped {
            socket: runtime_paths()?.sock,
        }),
        Err(err) => Err(err),
    }
}

fn is_daemon_socket_connect_error(err: &anyhow::Error) -> bool {
    err.chain()
        .any(|cause| cause.to_string().starts_with("connect daemon socket:"))
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

pub fn ensure_capture_blocking(workspace: String, deep: bool) -> Result<CaptureStatus> {
    match request_blocking(DaemonRequest::EnsureWorkspaceCapture { workspace, deep })? {
        DaemonResponse::CaptureStatus(status) => Ok(*status),
        DaemonResponse::Error { message, .. } => Err(anyhow!(message)),
        _ => Err(anyhow!("unexpected daemon capture response")),
    }
}

pub fn ensure_proxy_blocking(workspace: String, provider: String) -> Result<ProxyEndpoint> {
    match request_blocking(DaemonRequest::EnsureProxy {
        workspace,
        provider,
    })? {
        DaemonResponse::ProxyEndpoint(endpoint) => Ok(endpoint),
        DaemonResponse::Error { message, .. } => Err(anyhow!(message)),
        _ => Err(anyhow!("unexpected daemon proxy response")),
    }
}

pub fn begin_observed_session_blocking(
    workspace: String,
    agent: String,
) -> Result<ObservedSession> {
    match request_blocking(DaemonRequest::BeginObservedSession { workspace, agent })? {
        DaemonResponse::ObservedSession(session) => Ok(session),
        DaemonResponse::Error { message, .. } => Err(anyhow!(message)),
        _ => Err(anyhow!("unexpected daemon observe response")),
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
