// SPDX-License-Identifier: AGPL-3.0-or-later
//! Daemon server loop and single request worker.

use super::worker::{Job, spawn_worker};
use super::{RuntimePaths, runtime_paths};
use crate::ipc::{
    DaemonRequest, DaemonResponse, DaemonStatus, PROTO_VERSION, ServerHello, read_frame,
    write_frame,
};
use anyhow::{Context, Result, anyhow};
use std::fs::{File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{mpsc, oneshot};

#[derive(Clone)]
struct ServerState {
    started: Instant,
    queue_depth: Arc<AtomicUsize>,
    last_error: Arc<Mutex<Option<String>>>,
    tx: mpsc::Sender<Job>,
}

pub async fn run_server() -> Result<()> {
    let paths = runtime_paths()?;
    std::fs::create_dir_all(&paths.dir)?;
    let _pid_lock = lock_pid(&paths)?;
    remove_stale_socket(&paths.sock)?;
    let listener = UnixListener::bind(&paths.sock)
        .with_context(|| format!("bind daemon socket: {}", paths.sock.display()))?;
    set_socket_private(&paths.sock)?;
    let (tx, rx) = mpsc::channel(128);
    let state = ServerState {
        started: Instant::now(),
        queue_depth: Arc::new(AtomicUsize::new(0)),
        last_error: Arc::new(Mutex::new(None)),
        tx,
    };
    spawn_worker(rx, state.queue_depth.clone(), state.last_error.clone());
    loop {
        let (stream, _) = listener.accept().await?;
        let state = state.clone();
        tokio::spawn(async move {
            if let Err(err) = handle_client(stream, state).await {
                tracing::warn!(%err, "daemon client failed");
            }
        });
    }
}

fn lock_pid(paths: &RuntimePaths) -> Result<File> {
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&paths.pid)
        .with_context(|| format!("open pid file: {}", paths.pid.display()))?;
    file.try_lock()
        .map_err(|_| anyhow!("daemon already running: {}", paths.pid.display()))?;
    file.set_len(0)?;
    file.seek(SeekFrom::Start(0))?;
    writeln!(file, "{}", std::process::id())?;
    file.flush()?;
    Ok(file)
}

fn remove_stale_socket(sock: &PathBuf) -> Result<()> {
    if sock.exists() {
        std::fs::remove_file(sock)
            .with_context(|| format!("remove stale socket: {}", sock.display()))?;
    }
    Ok(())
}

#[cfg(unix)]
fn set_socket_private(sock: &PathBuf) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(sock, std::fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_socket_private(_sock: &PathBuf) -> Result<()> {
    Ok(())
}

async fn handle_client(mut stream: UnixStream, state: ServerState) -> Result<()> {
    let request: DaemonRequest = read_frame(&mut stream).await?;
    let response = match request {
        DaemonRequest::Hello(hello) if hello.proto_version != PROTO_VERSION => {
            DaemonResponse::Error {
                message: format!("unsupported proto_version {}", hello.proto_version),
                supported_min: Some(PROTO_VERSION),
                supported_max: Some(PROTO_VERSION),
            }
        }
        DaemonRequest::Hello(_) => DaemonResponse::Hello(ServerHello {
            proto_version: PROTO_VERSION,
            daemon_version: env!("CARGO_PKG_VERSION").to_string(),
            workspaces: crate::core::machine_registry::list_paths()
                .unwrap_or_default()
                .into_iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect(),
        }),
        DaemonRequest::Status => DaemonResponse::Status(status(&state)),
        request => run_job(&state, request).await?,
    };
    let stop = matches!(response, DaemonResponse::Ack { ref message } if message == "stopping");
    write_frame(&mut stream, &response).await?;
    if stop {
        std::process::exit(0);
    }
    Ok(())
}

fn status(state: &ServerState) -> DaemonStatus {
    DaemonStatus {
        pid: std::process::id(),
        uptime_ms: state.started.elapsed().as_millis() as u64,
        queue_depth: state.queue_depth.load(Ordering::Relaxed),
        last_error: state.last_error.lock().ok().and_then(|e| e.clone()),
    }
}

async fn run_job(state: &ServerState, request: DaemonRequest) -> Result<DaemonResponse> {
    let (reply, recv) = oneshot::channel();
    state.queue_depth.fetch_add(1, Ordering::Relaxed);
    state.tx.send(Job { request, reply }).await?;
    recv.await.map_err(Into::into)
}
