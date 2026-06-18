// SPDX-License-Identifier: AGPL-3.0-or-later
//! Daemon IPC client calls.

use crate::ipc::{
    CaptureStatus, ClientHello, ClientKind, DaemonRequest, DaemonResponse, ObservedSession,
    PROTO_VERSION, ProxyEndpoint, ServerHello, read_frame, write_frame,
};
use anyhow::{Context, Result, anyhow};
use tokio::net::UnixStream;

pub fn request_blocking(request: DaemonRequest) -> Result<DaemonResponse> {
    super::ensure_running()?;
    tokio::runtime::Runtime::new()?.block_on(request_async(request))
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

pub(super) async fn request_async(request: DaemonRequest) -> Result<DaemonResponse> {
    let paths = super::runtime_paths()?;
    let mut stream = UnixStream::connect(&paths.sock)
        .await
        .with_context(|| format!("connect daemon socket: {}", paths.sock.display()))?;
    write_frame(&mut stream, &request).await?;
    read_frame(&mut stream).await
}
