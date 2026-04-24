// SPDX-License-Identifier: AGPL-3.0-or-later
//! [PostHog](https://posthog.com/docs/api/capture) JSON capture, one `batch` request per flush:
//! one capture entry per **canonical** item (see `sync::canonical`).

use crate::sync::IngestExportBatch;
use crate::sync::canonical::{CanonicalItem, expand_ingest_batch};
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

fn build_batch_array(batch: &IngestExportBatch) -> Result<Vec<serde_json::Value>> {
    let expanded = expand_ingest_batch(batch);
    if expanded.is_empty() {
        return Ok(vec![]);
    }
    let did = first_distinct_id(&expanded)?;
    let mut v = Vec::with_capacity(expanded.len());
    for item in &expanded {
        v.push(capture_for_item(item, did)?);
    }
    Ok(v)
}

fn first_distinct_id(items: &[CanonicalItem]) -> Result<&str> {
    let first = items
        .first()
        .ok_or_else(|| anyhow::anyhow!("empty expand"))?;
    let h = match first {
        CanonicalItem::Event(e) => e.envelope.workspace_hash.as_str(),
        CanonicalItem::ToolSpan(t) => t.envelope.workspace_hash.as_str(),
        CanonicalItem::RepoSnapshotChunk(c) => c.envelope.workspace_hash.as_str(),
        CanonicalItem::WorkspaceFactSnapshot { envelope, .. } => envelope.workspace_hash.as_str(),
    };
    Ok(h)
}

fn capture_for_item(item: &CanonicalItem, distinct_id: &str) -> Result<serde_json::Value> {
    match item {
        CanonicalItem::Event(e) => phantom_event("kaizen.event", &e.event, distinct_id),
        CanonicalItem::ToolSpan(t) => {
            let span = serde_json::to_value(&t.span)?;
            Ok(serde_json::json!({
                "event": "kaizen.tool_span",
                "distinct_id": distinct_id,
                "properties": {
                    "kaizen_schema_version": t.envelope.kaizen_schema_version,
                    "team_id": &t.envelope.team_id,
                    "workspace_hash": &t.envelope.workspace_hash,
                    "span": span,
                }
            }))
        }
        CanonicalItem::RepoSnapshotChunk(c) => {
            let chunk = serde_json::to_value(&c.chunk)?;
            Ok(serde_json::json!({
                "event": "kaizen.repo_snapshot_chunk",
                "distinct_id": distinct_id,
                "properties": {
                    "kaizen_schema_version": c.envelope.kaizen_schema_version,
                    "team_id": &c.envelope.team_id,
                    "workspace_hash": &c.envelope.workspace_hash,
                    "chunk": chunk,
                }
            }))
        }
        CanonicalItem::WorkspaceFactSnapshot {
            envelope, payload, ..
        } => Ok(serde_json::json!({
            "event": "kaizen.workspace_fact_snapshot",
            "distinct_id": distinct_id,
            "properties": {
                "kaizen_schema_version": envelope.kaizen_schema_version,
                "team_id": &envelope.team_id,
                "workspace_hash": &envelope.workspace_hash,
                "payload": payload,
            }
        })),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync::IngestExportBatch;
    use crate::sync::outbound::EventsBatchBody;
    use crate::sync::outbound::OutboundEvent;

    #[test]
    fn one_capture_per_row_matches_expand() {
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
        let arr = build_batch_array(&b).unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["event"], "kaizen.event");
    }
}
