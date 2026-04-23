// SPDX-License-Identifier: AGPL-3.0-or-later
//! [Datadog Events API](https://docs.datadoghq.com/api/latest/events/) — one text event per batch (MVP).

use crate::sync::IngestExportBatch;
use crate::telemetry::TelemetryExporter;
use anyhow::Result;
use reqwest::blocking::Client;
use reqwest::header::HeaderValue;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(30);

pub struct DatadogExporter {
    client: Client,
    url: String,
    api_key: String,
}

impl DatadogExporter {
    pub fn new(site: &str, api_key: &str) -> Self {
        let s = site.trim();
        let url = format!("https://api.{s}/api/v1/events");
        let client = Client::builder()
            .timeout(TIMEOUT)
            .build()
            .expect("reqwest for Datadog");
        Self {
            client,
            url,
            api_key: api_key.to_string(),
        }
    }
}

impl TelemetryExporter for DatadogExporter {
    fn name(&self) -> &str {
        "datadog"
    }

    fn export(&self, batch: &IngestExportBatch) -> Result<()> {
        let (title, text, wh) = match batch {
            IngestExportBatch::Events(b) => (
                "kaizen events batch",
                format!("count={} team={}", b.events.len(), b.team_id),
                b.workspace_hash.as_str(),
            ),
            IngestExportBatch::ToolSpans(b) => (
                "kaizen tool_spans batch",
                format!("count={} team={}", b.spans.len(), b.team_id),
                b.workspace_hash.as_str(),
            ),
            IngestExportBatch::RepoSnapshots(b) => (
                "kaizen repo_snapshots batch",
                format!("count={} team={}", b.snapshots.len(), b.team_id),
                b.workspace_hash.as_str(),
            ),
        };
        let body = serde_json::json!({
            "title": title,
            "text": text,
            "tags": [
                "source:kaizen",
                format!("kind:{}", batch.kind_name()),
            ],
        });
        let mut key = HeaderValue::from_str(&self.api_key)
            .map_err(|e| anyhow::anyhow!("invalid Datadog API key: {e}"))?;
        key.set_sensitive(true);
        self.client
            .post(&self.url)
            .header("DD-API-KEY", key)
            .json(&body)
            .send()?
            .error_for_status()?;
        let _ = wh;
        Ok(())
    }
}
