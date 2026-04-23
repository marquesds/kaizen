// SPDX-License-Identifier: AGPL-3.0-or-later
//! [PostHog](https://posthog.com/docs/api/capture) JSON capture, one `batch` request per flush.

use crate::sync::IngestExportBatch;
use crate::sync::outbound::OutboundEvent;
use crate::telemetry::TelemetryExporter;
use anyhow::Result;
use reqwest::blocking::Client;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(30);

pub struct PostHogExporter {
    client: Client,
    capture_url: String,
    project_api_key: String,
}

impl PostHogExporter {
    pub fn new(host: &str, project_api_key: &str) -> Self {
        let h = host.trim_end_matches('/');
        let capture_url = format!("{h}/batch/");
        let client = Client::builder()
            .timeout(TIMEOUT)
            .build()
            .expect("reqwest client for PostHog");
        Self {
            client,
            capture_url,
            project_api_key: project_api_key.to_string(),
        }
    }
}

impl TelemetryExporter for PostHogExporter {
    fn name(&self) -> &str {
        "posthog"
    }

    fn export(&self, batch: &IngestExportBatch) -> Result<()> {
        let items = build_batch_array(batch)?;
        if items.is_empty() {
            return Ok(());
        }
        let body = serde_json::json!({
            "api_key": &self.project_api_key,
            "batch": items,
        });
        self.client
            .post(&self.capture_url)
            .json(&body)
            .send()?
            .error_for_status()?;
        Ok(())
    }
}

/// Map domain batches to PostHog batch entries (`distinct_id` = workspace hash only).
fn build_batch_array(batch: &IngestExportBatch) -> Result<Vec<serde_json::Value>> {
    match batch {
        IngestExportBatch::Events(b) => {
            let did = b.workspace_hash.as_str();
            let mut v = Vec::new();
            for e in &b.events {
                v.push(phantom_event("kaizen.event", e, did)?);
            }
            Ok(v)
        }
        IngestExportBatch::ToolSpans(b) => Ok(vec![serde_json::json!({
            "event": "kaizen.tool_spans",
            "distinct_id": b.workspace_hash,
            "properties": {
                "team_id": b.team_id,
                "workspace_hash": b.workspace_hash,
                "spans": b.spans,
            },
        })]),
        IngestExportBatch::RepoSnapshots(b) => Ok(vec![serde_json::json!({
            "event": "kaizen.repo_snapshots",
            "distinct_id": b.workspace_hash,
            "properties": {
                "team_id": b.team_id,
                "workspace_hash": b.workspace_hash,
                "snapshots": b.snapshots,
            },
        })]),
    }
}

fn phantom_event(name: &str, e: &OutboundEvent, distinct_id: &str) -> Result<serde_json::Value> {
    let props = serde_json::to_value(e)?;
    Ok(serde_json::json!({
        "event": name,
        "distinct_id": distinct_id,
        "properties": props,
    }))
}
