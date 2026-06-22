// SPDX-License-Identifier: AGPL-3.0-or-later
//! Embedded web assets.

pub mod brand;

use axum::Router;
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::response::IntoResponse;
use axum::routing::get;

pub const INDEX: &str = include_str!("assets/index.html");
pub const TOKENS: &str = include_str!("assets/kaizen-tokens.css");
pub const CSS: &str = include_str!("assets/kaizen.css");
pub const JS: &str = include_str!("assets/kaizen.js");
pub const STATE_JS: &str = include_str!("assets/kaizen-state.js");
pub const TRANSPORT_JS: &str = include_str!("assets/kaizen-transport.js");
pub const RENDER_JS: &str = include_str!("assets/kaizen-render.js");
pub const RAW_JS: &str = include_str!("assets/kaizen-raw.js");
pub const DETAIL_JS: &str = include_str!("assets/kaizen-detail.js");
pub const FORMAT_JS: &str = include_str!("assets/kaizen-format.js");
pub const SESSIONS_JS: &str = include_str!("assets/kaizen-sessions.js");
pub const SESSION_CONTROLS_JS: &str = include_str!("assets/kaizen-session-controls.js");
pub const SNAPSHOT_STATE_JS: &str = include_str!("assets/kaizen-snapshot-state.js");

pub fn session_router<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/assets/kaizen-format.js", get(format_js))
        .route("/assets/kaizen-sessions.js", get(sessions_js))
        .route(
            "/assets/kaizen-session-controls.js",
            get(session_controls_js),
        )
        .route("/assets/kaizen-snapshot-state.js", get(snapshot_state_js))
}

pub async fn index() -> impl IntoResponse {
    content("text/html; charset=utf-8", INDEX)
}

pub async fn tokens() -> impl IntoResponse {
    content("text/css; charset=utf-8", TOKENS)
}

pub async fn css() -> impl IntoResponse {
    content("text/css; charset=utf-8", CSS)
}

pub async fn js() -> impl IntoResponse {
    content("application/javascript", JS)
}

pub async fn state_js() -> impl IntoResponse {
    content("application/javascript", STATE_JS)
}

pub async fn transport_js() -> impl IntoResponse {
    content("application/javascript", TRANSPORT_JS)
}

pub async fn render_js() -> impl IntoResponse {
    content("application/javascript", RENDER_JS)
}

pub async fn raw_js() -> impl IntoResponse {
    content("application/javascript", RAW_JS)
}

pub async fn detail_js() -> impl IntoResponse {
    content("application/javascript", DETAIL_JS)
}

pub async fn format_js() -> impl IntoResponse {
    content("application/javascript", FORMAT_JS)
}

pub async fn sessions_js() -> impl IntoResponse {
    content("application/javascript", SESSIONS_JS)
}

pub async fn session_controls_js() -> impl IntoResponse {
    content("application/javascript", SESSION_CONTROLS_JS)
}

pub async fn snapshot_state_js() -> impl IntoResponse {
    content("application/javascript", SNAPSHOT_STATE_JS)
}

fn content(kind: &'static str, body: &'static str) -> impl IntoResponse {
    ([(CONTENT_TYPE, kind), (CACHE_CONTROL, "no-store")], body)
}

#[cfg(test)]
mod contract_tests;
#[cfg(test)]
mod tests;
