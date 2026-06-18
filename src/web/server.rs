// SPDX-License-Identifier: AGPL-3.0-or-later
//! Axum router for the local daemon web app.

use super::{assets, features, live::Subscription, snapshot, tools};
use axum::Router;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone)]
struct AppState {
    token: Arc<str>,
}

#[derive(Deserialize)]
struct TokenQuery {
    token: Option<String>,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClientMessage {
    Call {
        id: String,
        tool: String,
        #[serde(default)]
        args: Option<Value>,
    },
    Subscribe {
        #[serde(default)]
        id: Option<String>,
        #[serde(default)]
        workspace: Option<String>,
    },
    Unsubscribe,
    Ping {
        #[serde(default)]
        id: Option<String>,
    },
    VisualizationSnapshot {
        id: String,
        workspace: String,
        #[serde(default)]
        selected_session_id: Option<String>,
    },
}

pub fn router(token: String) -> Router {
    let state = AppState {
        token: Arc::from(token),
    };
    Router::new()
        .route("/", get(assets::index))
        .route("/assets/kaizen-tokens.css", get(assets::tokens))
        .route("/assets/kaizen.css", get(assets::css))
        .route("/assets/kaizen.js", get(assets::js))
        .route("/assets/kaizen-state.js", get(assets::state_js))
        .route("/assets/kaizen-transport.js", get(assets::transport_js))
        .route("/assets/kaizen-render.js", get(assets::render_js))
        .route("/assets/kaizen-raw.js", get(assets::raw_js))
        .route("/assets/kaizen-detail.js", get(assets::detail_js))
        .route("/assets/kaizen-format.js", get(assets::format_js))
        .route("/ws", get(ws))
        .with_state(state)
}

async fn ws(
    State(st): State<AppState>,
    Query(q): Query<TokenQuery>,
    ws: WebSocketUpgrade,
) -> Response {
    if q.token.as_deref() != Some(st.token.as_ref()) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    ws.on_upgrade(socket_loop).into_response()
}

async fn socket_loop(mut socket: WebSocket) {
    let mut subscription = Subscription::default();
    let mut tick = tokio::time::interval(std::time::Duration::from_millis(250));
    loop {
        tokio::select! {
            msg = socket.recv() => {
                let Some(Ok(Message::Text(text))) = msg else { break; };
                if !handle_text(&mut socket, &text, &mut subscription).await {
                    break;
                }
            }
            _ = tick.tick(), if subscription.is_active() => {
                if let Some(value) = subscription.changed()
                    && send(&mut socket, value).await.is_err() {
                    break;
                }
            }
        }
    }
}

async fn handle_text(socket: &mut WebSocket, text: &str, subscription: &mut Subscription) -> bool {
    match serde_json::from_str::<ClientMessage>(text) {
        Ok(ClientMessage::Call { id, tool, args }) => {
            let value = call_msg(&id, &tool, args.unwrap_or_else(|| json!({}))).await;
            send(socket, value).await.is_ok()
        }
        Ok(ClientMessage::Subscribe { id, workspace }) => {
            if let Err(err) = subscription.set(workspace) {
                return send(
                    socket,
                    json!({"type":"error","id":id,"error":err.to_string()}),
                )
                .await
                .is_ok();
            }
            send(socket, status_msg(id.as_deref())).await.is_ok()
        }
        Ok(ClientMessage::Unsubscribe) => {
            subscription.clear();
            send(
                socket,
                json!({"type":"result","output":{"kind":"json","value":{"subscribed":false}}}),
            )
            .await
            .is_ok()
        }
        Ok(ClientMessage::Ping { id }) => {
            send(socket, json!({"type":"pong","id":id})).await.is_ok()
        }
        Ok(ClientMessage::VisualizationSnapshot {
            id,
            workspace,
            selected_session_id,
        }) => {
            let value = snapshot_msg(id, workspace, selected_session_id).await;
            send(socket, value).await.is_ok()
        }
        Err(err) => send(socket, json!({"type":"error","error":err.to_string()}))
            .await
            .is_ok(),
    }
}

async fn call_msg(id: &str, tool: &str, args: Value) -> Value {
    match tools::call(tool, args).await {
        Ok(output) => json!({"type":"result","id":id,"tool":tool,"output":output}),
        Err(err) => json!({"type":"error","id":id,"tool":tool,"error":err}),
    }
}

async fn snapshot_msg(id: String, workspace: String, selected_session_id: Option<String>) -> Value {
    let req = snapshot::SnapshotRequest {
        workspace,
        selected_session_id,
    };
    match tokio::task::spawn_blocking(move || snapshot::load(req)).await {
        Ok(Ok(report)) => json!({"type":"visualization_snapshot","id":id,"report":report}),
        Ok(Err(err)) => json!({"type":"error","id":id,"error":err.to_string()}),
        Err(err) => json!({"type":"error","id":id,"error":err.to_string()}),
    }
}

fn status_msg(id: Option<&str>) -> Value {
    json!({
        "type": "status",
        "id": id,
        "ts_ms": now_ms(),
        "tools": tools::WEB_TOOL_NAMES,
        "features": features::all(),
    })
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or_default()
}

async fn send(socket: &mut WebSocket, value: Value) -> Result<(), axum::Error> {
    socket.send(Message::Text(value.to_string().into())).await
}
