//! Axum stub — minimal ingest server for integration testing.
//!
//! Routes:
//! - `GET  /v1/health`  → 200
//! - `POST /v1/events`  → 202 (new key) | 409 (duplicate)
//!
//! Idempotency key read from `X-Kaizen-Idempotency-Key` header.
//! When a gzip JSON body is present, optionally checks Bearer token and
//! rejects bodies that still contain raw `/Users/` or a test secret marker.

use axum::{
    Router,
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use flate2::read::GzDecoder;
use std::collections::HashSet;
use std::io::Read;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct IngestState {
    keys: Arc<Mutex<HashSet<String>>>,
    /// Decompressed JSON bodies accepted by the stub (for assertions).
    pub captured_bodies: Arc<Mutex<Vec<String>>>,
}

impl IngestState {
    pub fn new() -> Self {
        Self {
            keys: Arc::new(Mutex::new(HashSet::new())),
            captured_bodies: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

/// Build router + shared state.
pub fn router() -> (Router, IngestState) {
    let state = IngestState::new();
    let app = Router::new()
        .route("/v1/health", get(health))
        .route("/v1/events", post(ingest))
        .with_state(state.clone());
    (app, state)
}

async fn health() -> StatusCode {
    StatusCode::OK
}

pub const TEST_SECRET_MARKER: &str = "sk-super-secret-test";

async fn ingest(State(st): State<IngestState>, headers: HeaderMap, body: Bytes) -> Response {
    let key = headers
        .get("X-Kaizen-Idempotency-Key")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    if !body.is_empty() {
        let Some(auth) = headers.get("Authorization").and_then(|v| v.to_str().ok()) else {
            return StatusCode::UNAUTHORIZED.into_response();
        };
        if !auth.starts_with("Bearer ") {
            return StatusCode::UNAUTHORIZED.into_response();
        }
        let raw = if headers
            .get("Content-Encoding")
            .and_then(|v| v.to_str().ok())
            == Some("gzip")
        {
            let mut decoder = GzDecoder::new(&body[..]);
            let mut s = String::new();
            if decoder.read_to_string(&mut s).is_err() {
                return StatusCode::BAD_REQUEST.into_response();
            }
            s
        } else {
            String::from_utf8_lossy(&body).into_owned()
        };
        if raw.contains("/Users/") || raw.contains("/home/") || raw.contains(TEST_SECRET_MARKER) {
            return StatusCode::BAD_REQUEST.into_response();
        }
        st.captured_bodies.lock().unwrap().push(raw);
    }

    let mut lock = st.keys.lock().unwrap();
    if lock.contains(&key) {
        StatusCode::CONFLICT.into_response()
    } else {
        lock.insert(key);
        (StatusCode::ACCEPTED, r#"{"received":1,"deduped":0}"#).into_response()
    }
}
