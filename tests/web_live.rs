// SPDX-License-Identifier: AGPL-3.0-or-later

use futures_util::{SinkExt, StreamExt};
use kaizen::core::event::{Event, EventKind, EventSource, SessionRecord, SessionStatus};
use kaizen::store::Store;
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio::time::{Duration, timeout};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

#[tokio::test]
async fn subscribed_workspace_reports_database_changes_within_one_second() -> anyhow::Result<()> {
    let fixture = Fixture::new()?;
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let (endpoint, _task) = kaizen::web::start_with_listener(listener).await?;
    let (mut socket, _) = connect_async(fixture.ws_url(&endpoint)).await?;
    socket
        .send(Message::Text(fixture.subscribe().into()))
        .await?;
    assert_eq!(recv(&mut socket).await?["type"], "status");

    fixture.append_event()?;
    let changed = timeout(Duration::from_secs(1), recv(&mut socket)).await??;

    assert_eq!(changed["type"], "changed");
    assert_eq!(changed["workspace"], fixture.workspace_string());
    Ok(())
}

struct Fixture {
    _temp: tempfile::TempDir,
    workspace: std::path::PathBuf,
    store: Store,
}

impl Fixture {
    fn new() -> anyhow::Result<Self> {
        let temp = tempfile::tempdir()?;
        let workspace = temp.path().join("repo");
        std::fs::create_dir_all(&workspace)?;
        unsafe {
            std::env::set_var("HOME", temp.path());
            std::env::set_var("KAIZEN_HOME", temp.path().join(".kaizen"));
            std::env::set_var("KAIZEN_DAEMON", "0");
        }
        let workspace = std::fs::canonicalize(workspace)?;
        let store = Store::open(&kaizen::core::workspace::db_path(&workspace)?)?;
        store.upsert_session(&session(&workspace))?;
        Ok(Self {
            _temp: temp,
            workspace,
            store,
        })
    }

    fn ws_url(&self, endpoint: &kaizen::ipc::WebEndpoint) -> String {
        format!("ws://{}/ws?token={}", endpoint.listen, endpoint.token)
    }

    fn subscribe(&self) -> String {
        json!({"type":"subscribe", "id":"live", "workspace":self.workspace}).to_string()
    }

    fn append_event(&self) -> anyhow::Result<()> {
        self.store.append_event(&event())
    }

    fn workspace_string(&self) -> String {
        self.workspace.to_string_lossy().into_owned()
    }
}

fn session(workspace: &std::path::Path) -> SessionRecord {
    SessionRecord {
        id: "live-session".into(),
        agent: "claude".into(),
        model: None,
        workspace: workspace.to_string_lossy().into_owned(),
        started_at_ms: 1,
        ended_at_ms: None,
        status: SessionStatus::Running,
        trace_path: String::new(),
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

fn event() -> Event {
    Event {
        session_id: "live-session".into(),
        seq: 0,
        ts_ms: 2,
        ts_exact: true,
        kind: EventKind::ToolCall,
        source: EventSource::Hook,
        tool: Some("Read".into()),
        tool_call_id: Some("call-1".into()),
        tokens_in: None,
        tokens_out: None,
        reasoning_tokens: None,
        cost_usd_e6: None,
        stop_reason: None,
        latency_ms: None,
        ttft_ms: None,
        retry_count: None,
        context_used_tokens: None,
        context_max_tokens: None,
        cache_creation_tokens: None,
        cache_read_tokens: None,
        system_prompt_tokens: None,
        payload: json!({}),
    }
}

async fn recv(
    socket: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) -> anyhow::Result<Value> {
    let Some(Ok(Message::Text(text))) = socket.next().await else {
        anyhow::bail!("missing WebSocket message");
    };
    Ok(serde_json::from_str(&text)?)
}
