// SPDX-License-Identifier: AGPL-3.0-or-later
use futures_util::{SinkExt, StreamExt};
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
