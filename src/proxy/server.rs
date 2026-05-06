// SPDX-License-Identifier: AGPL-3.0-or-later
//! Local HTTP server: any path and method, forward to Anthropic base URL.

use crate::proxy::forward::run_forward_inner;
use crate::proxy::opts::ProxyRunOptions;
use crate::proxy::state::ProxyState;
use axum::Router;
use axum::http::{Request, StatusCode};
use axum::response::IntoResponse;
use axum::response::Response;
use axum::routing::any;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::DefaultBodyLimit;

/// Forward any method+path+query+body, buffer upstream response, append one store row.
async fn handle(
    axum::extract::State(st): axum::extract::State<Arc<ProxyState>>,
    request: Request<axum::body::Body>,
) -> axum::response::Response {
    match do_forward(&st, request).await {
        Ok(r) => r,
        Err(e) => (StatusCode::BAD_GATEWAY, e.to_string()).into_response(),
    }
}

async fn do_forward(
    st: &Arc<ProxyState>,
    request: Request<axum::body::Body>,
) -> Result<Response, anyhow::Error> {
    let (parts, body) = request.into_parts();
    let method = parts.method;
    let path = parts.uri.path().trim_start_matches('/').to_string();
    let q = parts.uri.query().unwrap_or("").to_string();
    let headers = &parts.headers;
    let body = axum::body::to_bytes(body, st.options.max_request_bytes as usize).await?;
    let path_ref = if path.is_empty() { "" } else { &path };
    run_forward_inner(st, method, path_ref, &q, headers, &body).await
}

/// Build `Client`, bind, run until the process is killed.
pub async fn run(
    options: Arc<ProxyRunOptions>,
    workspace: PathBuf,
    config: crate::core::config::Config,
) -> Result<(), anyhow::Error> {
    let store_path = crate::core::workspace::db_path(&workspace)?;
    let client = build_client(&options)?;
    let st = Arc::new(ProxyState {
        options: options.clone(),
        store_path,
        workspace: workspace.clone(),
        config: Arc::new(config),
        client,
    });
    let limit = usize::try_from(st.options.max_request_bytes).unwrap_or(usize::MAX);
    let app = Router::new()
        .route("/{*path}", any(handle))
        .layer(DefaultBodyLimit::max(limit))
        .with_state(st);
    let addr: SocketAddr = options
        .listen
        .parse()
        .map_err(|e: std::net::AddrParseError| {
            anyhow::anyhow!(r#"bad --listen (expected e.g. "127.0.0.1:3847"): {e}"#)
        })?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let seen = listener.local_addr()?;
    tracing::info!(addr = %seen, "kaizen LLM proxy listening (set ANTHROPIC_BASE_URL to this /)");
    axum::serve(listener, app).await?;
    Ok(())
}

fn build_client(o: &ProxyRunOptions) -> Result<reqwest::Client, reqwest::Error> {
    use std::time::Duration;
    let mut b = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(30))
        .timeout(Duration::from_secs(300));
    if !o.compress_transport {
        b = b.no_gzip();
    }
    b.build()
}
