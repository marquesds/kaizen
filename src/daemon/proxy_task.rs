// SPDX-License-Identifier: AGPL-3.0-or-later
//! Daemon-owned LLM proxy task startup.

use crate::ipc::ProxyEndpoint;
use crate::proxy::ProxyRunOptions;
use anyhow::{Context, Result, anyhow};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::Path;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

pub(super) async fn start_proxy(
    ws: &Path,
    provider: &str,
) -> Result<(ProxyEndpoint, JoinHandle<()>)> {
    let cfg = crate::core::config::load(ws)?;
    let preferred = preferred_listen(&cfg.proxy.listen, &cfg.proxy.provider, provider);
    let (listener, addr) = bind_proxy(&preferred).await?;
    let options = Arc::new(ProxyRunOptions::from_config_with_overrides(
        &cfg,
        Some(addr.to_string()),
        None,
        Some(provider.into()),
    ));
    let endpoint = proxy_endpoint(provider, addr);
    let ws = ws.to_path_buf();
    let task = tokio::spawn(async move {
        if let Err(err) = crate::proxy::run_with_listener(options, ws, cfg, listener).await {
            tracing::warn!(%err, "daemon proxy task stopped");
        }
    });
    Ok((endpoint, task))
}

pub(super) fn normalize_provider(provider: &str) -> Result<String> {
    match provider.to_ascii_lowercase().as_str() {
        "anthropic" | "openai" => Ok(provider.to_ascii_lowercase()),
        other => Err(anyhow!("unsupported proxy provider: {other}")),
    }
}

pub(super) fn providers_for_agent(agent: &str) -> Vec<&'static str> {
    match agent.to_ascii_lowercase().as_str() {
        "claude" | "anthropic" => vec!["anthropic"],
        "codex" | "openai" => vec!["openai"],
        _ => vec!["anthropic", "openai"],
    }
}

async fn bind_proxy(preferred: &str) -> Result<(TcpListener, SocketAddr)> {
    let addr: SocketAddr = preferred.parse().context("parse proxy listen address")?;
    match TcpListener::bind(addr).await {
        Ok(listener) => {
            let seen = listener.local_addr()?;
            Ok((listener, seen))
        }
        Err(err) => bind_fallback(addr)
            .await
            .with_context(|| format!("bind preferred proxy {preferred}: {err}")),
    }
}

async fn bind_fallback(addr: SocketAddr) -> Result<(TcpListener, SocketAddr)> {
    let listener = TcpListener::bind(SocketAddr::new(addr.ip(), 0)).await?;
    let seen = listener.local_addr()?;
    Ok((listener, seen))
}

fn preferred_listen(base: &str, configured: &str, provider: &str) -> String {
    if configured.eq_ignore_ascii_case(provider) {
        return base.into();
    }
    base.parse::<SocketAddr>()
        .map(|addr| SocketAddr::new(addr.ip(), 0).to_string())
        .unwrap_or_else(|_| "127.0.0.1:0".into())
}

fn proxy_endpoint(provider: &str, addr: SocketAddr) -> ProxyEndpoint {
    let base_url = format!("http://{}", public_addr(addr));
    ProxyEndpoint {
        provider: provider.into(),
        listen: addr.to_string(),
        v1_base_url: (provider == "openai").then(|| format!("{base_url}/v1")),
        base_url,
    }
}

fn public_addr(addr: SocketAddr) -> SocketAddr {
    if addr.ip().is_unspecified() {
        return SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), addr.port());
    }
    addr
}
