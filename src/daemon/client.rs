// SPDX-License-Identifier: AGPL-3.0-or-later
//! Daemon IPC client calls.

use crate::ipc::{
    CaptureStatus, ClientHello, ClientKind, DaemonRequest, DaemonResponse, ObservedSession,
    PROTO_VERSION, ProxyEndpoint, ServerHello, read_frame, write_frame,
};
use anyhow::{Context, Result, anyhow};
use std::path::Path;
use std::time::Duration;
use tokio::net::UnixStream;

const LIFECYCLE_TIMEOUT_MS: u64 = 500;

pub fn request_blocking(request: DaemonRequest) -> Result<DaemonResponse> {
    super::ensure_running()?;
    tokio::runtime::Runtime::new()?.block_on(request_async(request))
}

fn request_blocking_for(workspace: &str, request: DaemonRequest) -> Result<DaemonResponse> {
    super::ensure_running_for(Path::new(workspace))?;
    tokio::runtime::Runtime::new()?.block_on(request_async(request))
}

pub fn hello_blocking(client: ClientKind, workspace: Option<String>) -> Result<ServerHello> {
    let request = DaemonRequest::Hello(ClientHello {
        proto_version: PROTO_VERSION,
        client,
        workspace: workspace.clone(),
    });
    let response = match workspace.as_deref() {
        Some(workspace) => request_blocking_for(workspace, request),
        None => request_blocking(request),
    }?;
    match response {
        DaemonResponse::Hello(hello) => Ok(hello),
        DaemonResponse::Error { message, .. } => Err(anyhow!(message)),
        _ => Err(anyhow!("unexpected daemon hello response")),
    }
}

pub fn ensure_capture_blocking(workspace: String, deep: bool) -> Result<CaptureStatus> {
    let request = DaemonRequest::EnsureWorkspaceCapture {
        workspace: workspace.clone(),
        deep,
    };
    match request_blocking_for(&workspace, request)? {
        DaemonResponse::CaptureStatus(status) => Ok(*status),
        DaemonResponse::Error { message, .. } => Err(anyhow!(message)),
        _ => Err(anyhow!("unexpected daemon capture response")),
    }
}

pub fn ensure_proxy_blocking(workspace: String, provider: String) -> Result<ProxyEndpoint> {
    let request = DaemonRequest::EnsureProxy {
        workspace: workspace.clone(),
        provider,
    };
    match request_blocking_for(&workspace, request)? {
        DaemonResponse::ProxyEndpoint(endpoint) => Ok(endpoint),
        DaemonResponse::Error { message, .. } => Err(anyhow!(message)),
        _ => Err(anyhow!("unexpected daemon proxy response")),
    }
}

pub fn begin_observed_session_blocking(
    workspace: String,
    agent: String,
) -> Result<ObservedSession> {
    let request = DaemonRequest::BeginObservedSession {
        workspace: workspace.clone(),
        agent,
    };
    match request_blocking_for(&workspace, request)? {
        DaemonResponse::ObservedSession(session) => Ok(session),
        DaemonResponse::Error { message, .. } => Err(anyhow!(message)),
        _ => Err(anyhow!("unexpected daemon observe response")),
    }
}

pub(super) async fn request_async(request: DaemonRequest) -> Result<DaemonResponse> {
    let paths = super::runtime_paths()?;
    let mut stream = UnixStream::connect(&paths.sock)
        .await
        .with_context(|| format!("connect daemon socket: {}", paths.sock.display()))?;
    write_frame(&mut stream, &request).await?;
    read_frame(&mut stream).await
}

pub(super) async fn request_lifecycle_async(request: DaemonRequest) -> Result<DaemonResponse> {
    let timeout = Duration::from_millis(LIFECYCLE_TIMEOUT_MS);
    tokio::time::timeout(timeout, request_async(request))
        .await
        .map_err(|_| anyhow!("daemon IPC timed out after {LIFECYCLE_TIMEOUT_MS}ms"))?
}
