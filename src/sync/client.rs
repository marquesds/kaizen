//! HTTP sync client: gzip JSON, retries, batch split on 413.

use crate::sync::outbound::EventsBatchBody;
use crate::sync::smart::{RepoSnapshotsBatchBody, ToolSpansBatchBody};
use anyhow::{Context, Result};
use flate2::Compression;
use flate2::write::GzEncoder;
use reqwest::blocking::Client;
use reqwest::header::{AUTHORIZATION, CONTENT_ENCODING, CONTENT_TYPE, RETRY_AFTER};
use std::io::Write;
use std::time::Duration;
use uuid::Uuid;

pub const CLIENT_HEADER_VALUE: &str = concat!("kaizen/", env!("CARGO_PKG_VERSION"));

pub enum PostBatchOutcome {
    Accepted { received: u64, deduped: u64 },
    Conflict,
    TooLarge,
    RateLimited(Duration),
    Unauthorized,
    ClientError(u16),
    ServerError(u16),
}

pub struct SyncHttpClient {
    http: Client,
    endpoint: String,
    team_token: String,
}

impl SyncHttpClient {
    pub fn new(endpoint: &str, team_token: &str) -> Result<Self> {
        let http = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .context("reqwest client")?;
        Ok(Self {
            http,
            endpoint: endpoint.trim_end_matches('/').to_string(),
            team_token: team_token.to_string(),
        })
    }

    /// POST one batch. `idempotency_key` is a fresh UUIDv7 per attempt when retrying after failure.
    pub fn post_events_batch(
        &self,
        body: &EventsBatchBody,
        idempotency_key: &Uuid,
    ) -> Result<PostBatchOutcome> {
        self.post_json_gzip("/v1/events", body, idempotency_key)
    }

    pub fn post_tool_spans_batch(
        &self,
        body: &ToolSpansBatchBody,
        idempotency_key: &Uuid,
    ) -> Result<PostBatchOutcome> {
        self.post_json_gzip("/v1/tool-spans", body, idempotency_key)
    }

    pub fn post_repo_snapshots_batch(
        &self,
        body: &RepoSnapshotsBatchBody,
        idempotency_key: &Uuid,
    ) -> Result<PostBatchOutcome> {
        self.post_json_gzip("/v1/repo-snapshots", body, idempotency_key)
    }

    fn post_json_gzip<T: serde::Serialize>(
        &self,
        path: &str,
        body: &T,
        idempotency_key: &Uuid,
    ) -> Result<PostBatchOutcome> {
        let json = serde_json::to_vec(body).context("serialize batch")?;
        let mut enc = GzEncoder::new(Vec::new(), Compression::default());
        enc.write_all(&json).context("gzip write")?;
        let gz = enc.finish().context("gzip finish")?;

        let url = format!("{}{}", self.endpoint, path);
        let resp = self
            .http
            .post(&url)
            .header(AUTHORIZATION, format!("Bearer {}", self.team_token))
            .header(CONTENT_TYPE, "application/json")
            .header(CONTENT_ENCODING, "gzip")
            .header("X-Kaizen-Idempotency-Key", idempotency_key.to_string())
            .header("X-Kaizen-Client", CLIENT_HEADER_VALUE)
            .body(gz)
            .send()
            .with_context(|| format!("POST {path}"))?;

        let status = resp.status();
        if status.as_u16() == 202 {
            let bytes = resp.bytes().unwrap_or_default();
            let v: serde_json::Value = if bytes.is_empty() {
                serde_json::json!({})
            } else {
                serde_json::from_slice(&bytes).unwrap_or(serde_json::json!({}))
            };
            let received = v.get("received").and_then(|x| x.as_u64()).unwrap_or(0);
            let deduped = v.get("deduped").and_then(|x| x.as_u64()).unwrap_or(0);
            return Ok(PostBatchOutcome::Accepted { received, deduped });
        }
        if status.as_u16() == 409 {
            return Ok(PostBatchOutcome::Conflict);
        }
        if status.as_u16() == 413 {
            return Ok(PostBatchOutcome::TooLarge);
        }
        if status.as_u16() == 429 {
            let d = retry_after_duration(resp.headers().get(RETRY_AFTER));
            return Ok(PostBatchOutcome::RateLimited(d));
        }
        if status.as_u16() == 401 {
            return Ok(PostBatchOutcome::Unauthorized);
        }
        let code = status.as_u16();
        if status.is_client_error() {
            return Ok(PostBatchOutcome::ClientError(code));
        }
        if status.is_server_error() {
            return Ok(PostBatchOutcome::ServerError(code));
        }
        Ok(PostBatchOutcome::ServerError(code))
    }

    pub fn health(&self) -> Result<bool> {
        let url = format!("{}/v1/health", self.endpoint);
        let resp = self.http.get(&url).send().context("GET /v1/health")?;
        Ok(resp.status().is_success())
    }
}

fn retry_after_duration(h: Option<&reqwest::header::HeaderValue>) -> Duration {
    let Some(h) = h else {
        return Duration::from_secs(2);
    };
    let s = h.to_str().unwrap_or("2");
    if let Ok(secs) = s.parse::<u64>() {
        return Duration::from_secs(secs.max(1));
    }
    Duration::from_secs(2)
}
