// SPDX-License-Identifier: AGPL-3.0-or-later
use futures_util::{SinkExt, StreamExt};
use kaizen::core::event::{SessionRecord, SessionStatus};
use kaizen::store::Store;
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Error as WsError;
use tokio_tungstenite::tungstenite::Message;

#[tokio::test]
async fn websocket_auth_and_tool_calls() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let home = tmp.path().join("home");
    let workspace = tmp.path().join("repo");
    std::fs::create_dir_all(&home)?;
    std::fs::create_dir_all(&workspace)?;
    let workspace = std::fs::canonicalize(workspace)?;
    unsafe {
        std::env::set_var("HOME", &home);
        std::env::set_var("KAIZEN_HOME", home.join(".kaizen"));
        std::env::set_var("KAIZEN_DAEMON", "0");
    }

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let (endpoint, _task) = kaizen::web::start_with_listener(listener).await?;
    assert_rejected(&format!("ws://{}/ws", endpoint.listen)).await?;
    assert_rejected(&format!("ws://{}/ws?token=wrong", endpoint.listen)).await?;

    let ws_url = format!("ws://{}/ws?token={}", endpoint.listen, endpoint.token);
    let (mut ws, _) = connect_async(ws_url).await?;
    ws.send(Message::Text(
        json!({"type":"ping","id":"p1"}).to_string().into(),
    ))
    .await?;
    assert_eq!(recv_type(&mut ws).await?, "pong");

    ws.send(Message::Text(call("bad", "nope", json!({})).into()))
        .await?;
    assert_eq!(recv_type(&mut ws).await?, "error");

    ws.send(Message::Text(
        call("cap", "kaizen_capabilities", json!({})).into(),
    ))
    .await?;
    let msg = recv_json(&mut ws).await?;
    assert_eq!(msg["type"], "result");
    assert_eq!(msg["output"]["kind"], "text");

    ws.send(Message::Text(call("tui", "kaizen_tui", json!({})).into()))
        .await?;
    let msg = recv_json(&mut ws).await?;
    assert_eq!(msg["type"], "error");
    assert!(msg["error"].as_str().unwrap_or("").contains("interactive"));

    ws.send(Message::Text(
        call(
            "init",
            "kaizen_init",
            json!({ "workspace": workspace.to_string_lossy() }),
        )
        .into(),
    ))
    .await?;
    let msg = recv_json(&mut ws).await?;
    assert_eq!(msg["type"], "result");
    assert!(
        msg["output"]["value"]
            .as_str()
            .unwrap_or("")
            .contains("kaizen init complete")
    );

    ws.send(Message::Text(
        call(
            "sessions",
            "kaizen_sessions_list",
            json!({ "workspace": workspace.to_string_lossy(), "json": true }),
        )
        .into(),
    ))
    .await?;
    let msg = recv_json(&mut ws).await?;
    assert_eq!(msg["type"], "result");
    assert_eq!(msg["output"]["kind"], "json");

    let store = Store::open(&kaizen::core::workspace::db_path(&workspace)?)?;
    store.upsert_session(&session(
        "web-session",
        workspace.to_string_lossy().as_ref(),
    ))?;
    ws.send(Message::Text(
        json!({
            "type": "visualization_snapshot",
            "id": "viz",
            "workspace": workspace.to_string_lossy(),
            "selected_session_id": "web-session"
        })
        .to_string()
        .into(),
    ))
    .await?;
    let msg = recv_json(&mut ws).await?;
    assert_eq!(msg["type"], "visualization_snapshot");
    assert_eq!(msg["id"], "viz");
    assert_eq!(msg["report"]["totals"]["session_count"], 1);
    assert_eq!(msg["report"]["selected"]["session"]["id"], "web-session");

    ws.send(Message::Text(
        json!({"type":"subscribe","id":"s1"}).to_string().into(),
    ))
    .await?;
    let msg = recv_json(&mut ws).await?;
    assert_eq!(msg["type"], "status");
    assert_eq!(msg["features"].as_array().unwrap().len(), 40);
    assert!(msg["features"].as_array().unwrap().iter().any(|feature| {
        feature["tool"] == "get_session_span_tree" && feature["label"] == "Show span tree"
    }));
    assert!(msg["features"].as_array().unwrap().iter().all(|f| {
        f["label"]
            .as_str()
            .is_some_and(|label| !label.contains("kaizen_"))
    }));
    Ok(())
}

async fn assert_rejected(url: &str) -> anyhow::Result<()> {
    let Err(WsError::Http(response)) = connect_async(url).await.map(|_| ()) else {
        anyhow::bail!("expected websocket HTTP rejection for {url}");
    };
    assert_eq!(response.status(), 401);
    Ok(())
}

fn call(id: &str, tool: &str, args: Value) -> String {
    json!({ "type": "call", "id": id, "tool": tool, "args": args }).to_string()
}

fn session(id: &str, workspace: &str) -> SessionRecord {
    SessionRecord {
        id: id.into(),
        agent: "codex".into(),
        model: Some("gpt-5".into()),
        workspace: workspace.into(),
        started_at_ms: 1_000,
        ended_at_ms: None,
        status: SessionStatus::Running,
        trace_path: "/trace".into(),
        start_commit: None,
        end_commit: None,
        branch: None,
        dirty_start: None,
        dirty_end: None,
        repo_binding_source: None,
        prompt_fingerprint: None,
        parent_session_id: None,
        agent_version: None,
        os: None,
        arch: None,
        repo_file_count: None,
        repo_total_loc: None,
    }
}

async fn recv_type(
    ws: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) -> anyhow::Result<String> {
    Ok(recv_json(ws).await?["type"]
        .as_str()
        .unwrap_or("")
        .to_string())
}

async fn recv_json(
    ws: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) -> anyhow::Result<Value> {
    let Some(Ok(Message::Text(text))) = ws.next().await else {
        anyhow::bail!("missing websocket text message");
    };
    Ok(serde_json::from_str(&text)?)
}
