// SPDX-License-Identifier: AGPL-3.0-or-later
//! [PostHog](https://posthog.com/docs/api/capture) JSON capture, one `batch` request per flush:
//! one capture entry per **canonical** item (see `sync::canonical`).

use crate::sync::IngestExportBatch;
use crate::sync::canonical::{CanonicalEnvelope, CanonicalItem, expand_ingest_batch};
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
        CanonicalItem::Event(e) => {
            phantom_event("kaizen.event", &e.event, &e.envelope, distinct_id)
        }
        CanonicalItem::ToolSpan(t) => {
            let span = serde_json::to_value(&t.span)?;
            Ok(capture(
                "kaizen.tool_span",
                envelope_props(&t.envelope, "span", span),
                distinct_id,
            ))
        }
        CanonicalItem::RepoSnapshotChunk(c) => {
            let chunk = serde_json::to_value(&c.chunk)?;
            Ok(capture(
                "kaizen.repo_snapshot_chunk",
                envelope_props(&c.envelope, "chunk", chunk),
                distinct_id,
            ))
        }
        CanonicalItem::WorkspaceFactSnapshot {
            envelope, payload, ..
        } => Ok(capture(
            "kaizen.workspace_fact_snapshot",
            envelope_props(envelope, "payload", serde_json::to_value(payload)?),
            distinct_id,
        )),
    }
}

fn phantom_event(
    name: &str,
    e: &OutboundEvent,
    envelope: &CanonicalEnvelope,
    distinct_id: &str,
) -> Result<serde_json::Value> {
    let mut props = serde_json::to_value(e)?;
    merge_project_name(&mut props, envelope);
    Ok(capture(name, props, distinct_id))
}

fn capture(name: &str, props: serde_json::Value, distinct_id: &str) -> serde_json::Value {
    serde_json::json!({
        "event": name,
        "distinct_id": distinct_id,
        "properties": props,
    })
}

fn envelope_props(
    envelope: &CanonicalEnvelope,
    key: &str,
    value: serde_json::Value,
) -> serde_json::Value {
    let mut props = serde_json::json!({
        "kaizen_schema_version": envelope.kaizen_schema_version,
        "team_id": &envelope.team_id,
        "workspace_hash": &envelope.workspace_hash,
        key: value,
    });
    merge_project_name(&mut props, envelope);
    props
}

fn merge_project_name(props: &mut serde_json::Value, envelope: &CanonicalEnvelope) {
    if let Some(name) = project_name(envelope)
        && let Some(map) = props.as_object_mut()
    {
        map.insert("project_name".into(), name.into());
    }
}

fn project_name(envelope: &CanonicalEnvelope) -> Option<&str> {
    envelope
        .identity
        .as_ref()
        .and_then(|i| i.workspace_label.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync::IngestExportBatch;
    use crate::sync::outbound::{EventsBatchBody, OutboundEvent};

    #[test]
    fn one_capture_per_row_matches_expand() {
        let b = IngestExportBatch::Events(EventsBatchBody {
            team_id: "t".into(),
            workspace_hash: "w".into(),
            project_name: Some("kaizen".into()),
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
        assert_eq!(arr[0]["properties"]["project_name"], "kaizen");
    }
}
