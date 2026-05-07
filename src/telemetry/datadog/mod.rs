// SPDX-License-Identifier: AGPL-3.0-or-later
//! Datadog Logs API exporter ([docs](https://docs.datadoghq.com/api/latest/logs/)).
//!
//! Architecture: pure JSON build + chunking in `build`, HTTP POST in `transport`. The
//! exporter is the boundary: it expands a redacted batch, builds DD-shaped log objects with
//! `timestamp` and `hostname`, chunks under DD's 1000-entry / 5 MB request caps, then fans
//! the chunks to the intake.

mod build;
mod transport;

use crate::sync::IngestExportBatch;
use crate::telemetry::TelemetryExporter;
use anyhow::Result;
use reqwest::blocking::Client;
use std::time::Duration;

/// Re-export of the pure DD log builder for the `tests/spec/telemetry_exporters.rs` connect
/// driver, so the spec invariant `dd_records_well_formed` checks the *real* JSON shape.
pub fn dd_log_object_for_test(
    item: &crate::sync::canonical::CanonicalItem,
    hostname: &str,
) -> serde_json::Value {
    build::dd_log_object(item, hostname)
}

const TIMEOUT: Duration = Duration::from_secs(30);

pub struct DatadogExporter {
    client: Client,
    /// `https://http-intake.logs.<site>/api/v2/logs`
    logs_url: String,
    api_key: String,
    hostname: String,
}

impl DatadogExporter {
    pub fn new(site: &str, api_key: &str) -> Self {
        let s = site.trim();
        let logs_url = format!("https://http-intake.logs.{s}/api/v2/logs");
        let client = Client::builder()
            .timeout(TIMEOUT)
            .build()
            .expect("reqwest for Datadog");
        Self {
            client,
            logs_url,
            api_key: api_key.to_string(),
            hostname: build::current_hostname(),
        }
    }
}

impl TelemetryExporter for DatadogExporter {
    fn name(&self) -> &str {
        "datadog"
    }

    fn export(&self, batch: &IngestExportBatch) -> Result<()> {
        let logs = build::build_log_objects(batch, &self.hostname);
        if logs.is_empty() {
            return Ok(());
        }
        let chunks = build::chunk_for_dd(logs);
        transport::post_chunks(&self.client, &self.logs_url, &self.api_key, chunks)
    }
}

#[cfg(test)]
mod tests {
    use super::build::{chunk_for_dd, dd_log_object};
    use crate::sync::canonical::expand_ingest_batch;
    use crate::sync::outbound::{EventsBatchBody, OutboundEvent};
    use crate::sync::{IngestExportBatch, smart::OutboundToolSpan, smart::ToolSpansBatchBody};

    fn one_event_batch() -> IngestExportBatch {
        IngestExportBatch::Events(EventsBatchBody {
            team_id: "t".into(),
            workspace_hash: "wh".into(),
            project_name: Some("kaizen".into()),
            events: vec![OutboundEvent {
                session_id_hash: "sid".into(),
                event_seq: 0,
                ts_ms: 1_700_000_000_000,
                agent: "cursor".into(),
                model: "gpt-5".into(),
                kind: "message".into(),
                source: "hook".into(),
                tool: Some("Edit".into()),
                tool_call_id: None,
                tokens_in: Some(120),
                tokens_out: Some(30),
                reasoning_tokens: None,
                cost_usd_e6: Some(900),
                payload: serde_json::json!({}),
            }],
        })
    }

    #[test]
    fn dd_log_object_has_timestamp_and_hostname_top_level() {
        let b = one_event_batch();
        let items = expand_ingest_batch(&b);
        let v = dd_log_object(&items[0], "host-1");
        assert_eq!(v["timestamp"], serde_json::json!(1_700_000_000_000_i64));
        assert_eq!(v["hostname"], serde_json::json!("host-1"));
        assert_eq!(v["agent"], serde_json::json!("cursor"));
        assert_eq!(v["model"], serde_json::json!("gpt-5"));
        assert_eq!(v["project_name"], serde_json::json!("kaizen"));
        assert_eq!(v["tokens_in"], serde_json::json!(120));
        let tags = v["ddtags"].as_str().unwrap();
        assert!(tags.contains("agent:cursor"));
        assert!(tags.contains("model:gpt-5"));
        assert!(tags.contains("project_name:kaizen"));
        assert!(tags.contains("kaizen.type:kaizen.event"));
        // Full canonical item nested under `kaizen` (not double-encoded as a string).
        assert!(v["kaizen"].is_object());
        assert!(v["message"].is_string());
    }

    #[test]
    fn dd_log_object_handles_tool_span_timestamp_fallback() {
        let b = IngestExportBatch::ToolSpans(ToolSpansBatchBody {
            team_id: "t".into(),
            workspace_hash: "wh".into(),
            project_name: Some("kaizen".into()),
            spans: vec![OutboundToolSpan {
                session_id_hash: "sid".into(),
                span_id_hash: "ph".into(),
                tool: Some("Read".into()),
                status: "ok".into(),
                started_at_ms: None,
                ended_at_ms: Some(42),
                lead_time_ms: Some(40),
                tokens_in: Some(10),
                tokens_out: Some(4),
                reasoning_tokens: Some(2),
                cost_usd_e6: Some(25),
                path_hashes: vec![],
            }],
        });
        let items = expand_ingest_batch(&b);
        let v = dd_log_object(&items[0], "h");
        assert_eq!(v["timestamp"], serde_json::json!(42_i64));
        assert_eq!(v["status"], serde_json::json!("ok"));
        assert_eq!(v["lead_time_ms"], serde_json::json!(40));
        assert_eq!(v["tokens_in"], serde_json::json!(10));
        assert_eq!(v["tokens_out"], serde_json::json!(4));
        assert_eq!(v["reasoning_tokens"], serde_json::json!(2));
        assert_eq!(v["cost_usd_e6"], serde_json::json!(25));
        assert_eq!(v["project_name"], serde_json::json!("kaizen"));
    }

    #[test]
    fn chunk_for_dd_respects_item_cap() {
        let logs: Vec<_> = (0..2_500).map(|i| serde_json::json!({"i": i})).collect();
        let chunks = chunk_for_dd(logs);
        assert_eq!(chunks.len(), 3);
        assert!(chunks.iter().all(|c| c.len() <= 1000));
        assert_eq!(chunks.iter().map(|c| c.len()).sum::<usize>(), 2_500);
    }

    #[test]
    fn chunk_for_dd_respects_byte_cap() {
        // ~64 KB string per item -> 100 items > 5 MB -> at least 2 chunks.
        let big = "x".repeat(64 * 1024);
        let logs: Vec<_> = (0..100).map(|_| serde_json::json!({"s": big})).collect();
        let chunks = chunk_for_dd(logs);
        assert!(chunks.len() >= 2);
        for c in &chunks {
            let bytes = serde_json::to_vec(c).unwrap().len();
            assert!(
                bytes <= super::build::MAX_BYTES_PER_CHUNK + 64 * 1024,
                "chunk too big: {bytes} bytes"
            );
        }
    }
}
