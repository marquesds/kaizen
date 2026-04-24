// SPDX-License-Identifier: AGPL-3.0-or-later
//! [Datadog Logs API v2](https://docs.datadoghq.com/api/latest/logs/) — one JSON log object per
//! canonical item (no Events API).

use crate::sync::IngestExportBatch;
use crate::sync::canonical::expand_ingest_batch;
use crate::telemetry::TelemetryExporter;
use anyhow::Result;
use reqwest::blocking::Client;
use reqwest::header::HeaderValue;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(30);

pub struct DatadogExporter {
    client: Client,
    /// `https://http-intake.logs.<site>/api/v2/logs`
    logs_url: String,
    api_key: String,
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
        }
    }
}

impl TelemetryExporter for DatadogExporter {
    fn name(&self) -> &str {
        "datadog"
    }

    fn export(&self, batch: &IngestExportBatch) -> Result<()> {
        let expanded = expand_ingest_batch(batch);
        if expanded.is_empty() {
            return Ok(());
        }
        let body: Vec<serde_json::Value> = expanded
            .iter()
            .map(|c| {
                serde_json::json!({
                    "ddsource": "kaizen",
                    "service": "kaizen",
                    "ddtags": format!("source:kaizen,kaizen.type:{}", c.telemetry_kind()),
                    "message": serde_json::to_string(c).unwrap_or_else(|_| "{}".to_string()),
                })
            })
            .collect();
        let mut key = HeaderValue::from_str(&self.api_key)
            .map_err(|e| anyhow::anyhow!("invalid Datadog API key: {e}"))?;
        key.set_sensitive(true);
        self.client
            .post(&self.logs_url)
            .header("Content-Type", "application/json")
            .header("DD-API-KEY", key)
            .json(&body)
            .send()?
            .error_for_status()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync::IngestExportBatch;
    use crate::sync::outbound::EventsBatchBody;
    use crate::sync::outbound::OutboundEvent;

    #[test]
    fn logs_array_matches_expand_without_network() {
        let b = IngestExportBatch::Events(EventsBatchBody {
            team_id: "t".into(),
            workspace_hash: "w".into(),
            events: vec![OutboundEvent {
                session_id_hash: "a".into(),
                event_seq: 0,
                ts_ms: 1,
                agent: "x".into(),
                model: "m".into(),
                kind: "message".into(),
                source: "hook".into(),
                tool: None,
                tool_call_id: None,
                tokens_in: None,
                tokens_out: None,
                reasoning_tokens: None,
                cost_usd_e6: None,
                payload: serde_json::json!({}),
            }],
        });
        let expanded = expand_ingest_batch(&b);
        let body: Vec<serde_json::Value> = expanded
            .iter()
            .map(|c| {
                serde_json::json!({
                    "ddsource": "kaizen",
                    "service": "kaizen",
                    "ddtags": format!("source:kaizen,kaizen.type:{}", c.telemetry_kind()),
                    "message": serde_json::to_string(c).unwrap(),
                })
            })
            .collect();
        assert_eq!(body.len(), 1);
        assert_eq!(body[0]["ddsource"], "kaizen");
    }
}
