// SPDX-License-Identifier: AGPL-3.0-or-later
//! Local daemon web app: embedded UI plus WebSocket tool calls.

mod assets;
mod server;
pub mod tools;

use crate::ipc::WebEndpoint;
use anyhow::{Context, Result};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use uuid::Uuid;

const DEFAULT_LISTEN: &str = "127.0.0.1:7878";

pub async fn start() -> Result<(WebEndpoint, JoinHandle<()>)> {
    let listener = bind_loopback().await?;
    start_with_listener(listener).await
}

pub async fn start_with_listener(listener: TcpListener) -> Result<(WebEndpoint, JoinHandle<()>)> {
    let token = Uuid::now_v7().simple().to_string();
    start_with_token(listener, token).await
}

pub async fn start_with_token(
    listener: TcpListener,
    token: String,
) -> Result<(WebEndpoint, JoinHandle<()>)> {
    let addr = listener.local_addr()?;
    let endpoint = endpoint(addr, token);
    let app = server::router(endpoint.token.clone());
    let task = tokio::spawn(async move {
        if let Err(err) = axum::serve(listener, app).await {
            tracing::warn!(%err, "daemon web app stopped");
        }
    });
    Ok((endpoint, task))
}

async fn bind_loopback() -> Result<TcpListener> {
    match TcpListener::bind(DEFAULT_LISTEN).await {
        Ok(listener) => Ok(listener),
        Err(err) => bind_fallback()
            .await
            .with_context(|| format!("bind daemon web app at {DEFAULT_LISTEN}: {err}")),
    }
}

async fn bind_fallback() -> Result<TcpListener> {
    TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
        .await
        .map_err(Into::into)
}

fn endpoint(addr: SocketAddr, token: String) -> WebEndpoint {
    let public = public_addr(addr);
    WebEndpoint {
        listen: addr.to_string(),
        url: format!("http://{public}/?token={token}"),
        token,
    }
}

fn public_addr(addr: SocketAddr) -> SocketAddr {
    if addr.ip().is_unspecified() {
        return SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), addr.port());
    }
    addr
}
