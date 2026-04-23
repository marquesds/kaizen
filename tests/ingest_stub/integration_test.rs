// SPDX-License-Identifier: AGPL-3.0-or-later
#[path = "mod.rs"]
mod stub;

use axum::{body::Body, http::Request};
use tower::ServiceExt;

use kaizen::core::config::TelemetryConfig;
use kaizen::core::event::{Event, EventKind, EventSource, SessionRecord, SessionStatus};
use kaizen::store::Store;
use kaizen::sync::FlushExporters;

#[tokio::test]
async fn health_returns_200() {
    let (app, _store) = stub::router();
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/v1/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn unique_keys_accepted_dup_rejected() {
    let (app, _store) = stub::router();

    // POST 10 unique keys → all 202.
    for i in 0..10u32 {
        let req = Request::builder()
            .method("POST")
            .uri("/v1/events")
            .header("X-Kaizen-Idempotency-Key", format!("key-{i}"))
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), 202, "key-{i} should be accepted");
    }

    // Replay key-5 → 409.
    let dup = Request::builder()
        .method("POST")
        .uri("/v1/events")
        .header("X-Kaizen-Idempotency-Key", "key-5")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(dup).await.unwrap();
    assert_eq!(resp.status(), 409, "duplicate key-5 must be rejected");
}

#[tokio::test]
async fn sync_flush_sends_redacted_gzip_batch() {
    let (app, state) = stub::router();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;

    let tmp = tempfile::TempDir::new().unwrap();
    let ws = tmp.path();
    std::fs::create_dir_all(ws.join(".kaizen")).unwrap();
    let salt_hex = "00".repeat(32);
    let endpoint = format!("http://{}", addr);
    std::fs::write(
        ws.join(".kaizen/config.toml"),
        format!(
            r#"[sync]
endpoint = "{endpoint}"
team_token = "test-token"
team_id = "team-1"
team_salt_hex = "{salt_hex}"
"#
        ),
    )
    .unwrap();

    let cfg = kaizen::core::config::load(ws).unwrap();
    let salt = kaizen::core::config::try_team_salt(&cfg.sync).unwrap();
    let ctx = kaizen::sync::ingest_ctx(&cfg, ws.to_path_buf()).unwrap();

    let db = ws.join(".kaizen/kaizen.db");
    let store = Store::open(&db).unwrap();
    let session = SessionRecord {
        id: "sess-1".into(),
        agent: "cursor".into(),
        model: Some("claude".into()),
        workspace: ws.to_string_lossy().into(),
        started_at_ms: 1,
        ended_at_ms: None,
        status: SessionStatus::Running,
        trace_path: "".into(),
        start_commit: None,
        end_commit: None,
        branch: None,
        dirty_start: None,
        dirty_end: None,
        repo_binding_source: None,
    };
    store.upsert_session(&session).unwrap();

    let ev = Event {
        session_id: "sess-1".into(),
        seq: 0,
        ts_ms: 99,
        ts_exact: true,
        kind: EventKind::ToolCall,
        source: EventSource::Hook,
        tool: Some("bash".into()),
        tool_call_id: Some("call-1".into()),
        tokens_in: None,
        tokens_out: None,
        reasoning_tokens: None,
        cost_usd_e6: None,
        payload: serde_json::json!({
            "path": "/Users/alice/proj/secret.txt",
            "token": stub::TEST_SECRET_MARKER,
        }),
    };
    store.append_event_with_sync(&ev, Some(&ctx)).unwrap();

    let db_path = db.clone();
    let ws_path = ws.to_path_buf();
    let sync_cfg = cfg.sync.clone();
    tokio::task::spawn_blocking(move || {
        let store = Store::open(&db_path).unwrap();
        let flush = FlushExporters {
            telemetry: &TelemetryConfig::default(),
            registry: None,
        };
        kaizen::sync::flush_outbox_once(&store, &ws_path, &sync_cfg, &salt, &flush).unwrap();
    })
    .await
    .unwrap();

    let bodies = state.captured_bodies.lock().unwrap();
    assert_eq!(bodies.len(), 2, "expected event + tool-span POST bodies");

    let parsed = bodies
        .iter()
        .map(|body| serde_json::from_str::<serde_json::Value>(body).unwrap())
        .collect::<Vec<_>>();

    let event_body = parsed
        .iter()
        .find(|body| body.get("events").is_some())
        .expect("events batch");
    let tool_body = parsed
        .iter()
        .find(|body| body.get("spans").is_some())
        .expect("tool spans batch");

    let events = event_body["events"].as_array().expect("events array");
    assert_eq!(events.len(), 1);
    let payload_str = events[0]["payload"].to_string();
    assert!(
        !payload_str.contains("/Users/"),
        "payload leaked path: {payload_str}"
    );
    assert!(
        !payload_str.contains("sk-super-secret"),
        "payload leaked secret: {payload_str}"
    );

    let spans = tool_body["spans"].as_array().expect("spans array");
    assert_eq!(spans.len(), 1);
    let span_str = spans[0].to_string();
    assert!(
        !span_str.contains("/Users/"),
        "span leaked path: {span_str}"
    );
    assert!(
        !span_str.contains("sk-super-secret"),
        "span leaked secret: {span_str}"
    );
}
