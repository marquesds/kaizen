// SPDX-License-Identifier: AGPL-3.0-or-later
//! Per-item canonical telemetry payloads: expand [`IngestExportBatch`](crate::sync::IngestExportBatch)
//! for exporters, future provider pull, and goldens. Primary POST / outbox stay batch-oriented.

use crate::core::identity::ActorIdentity;
use crate::sync::outbound::OutboundEvent;
use crate::sync::smart::{OutboundRepoSnapshotChunk, OutboundToolSpan};
use serde::{Deserialize, Serialize};

/// Forward evolution marker on exported or pulled items (read old payloads only in migration tools).
pub const KAIZEN_SCHEMA_VERSION: u32 = 1;

/// Shared context on every expanded item; identity is `None` until session/workspace wiring fills it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanonicalEnvelope {
    pub kaizen_schema_version: u32,
    pub team_id: String,
    pub workspace_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identity: Option<ActorIdentity>,
}

/// One logical event name for third-party and docs (`print-schema` in a later phase).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanonicalEventName {
    /// One outbound event row.
    Event,
    /// One tool span.
    ToolSpan,
    /// One repo graph snapshot chunk.
    RepoSnapshotChunk,
    /// Skills / rules / workspace metadata (Phase 6 producer).
    WorkspaceFactSnapshot,
}

/// Fully expanded item for a single `OutboundEvent` row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventItem {
    pub envelope: CanonicalEnvelope,
    pub name: CanonicalEventName,
    pub event: OutboundEvent,
}

/// One tool span with batch context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpanItem {
    pub envelope: CanonicalEnvelope,
    pub name: CanonicalEventName,
    pub span: OutboundToolSpan,
}

/// One repo snapshot chunk with batch context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoSnapshotChunkItem {
    pub envelope: CanonicalEnvelope,
    pub name: CanonicalEventName,
    pub chunk: OutboundRepoSnapshotChunk,
}

/// Workspace-level facts (hashed skill/rule slugs from `.cursor/skills` and `.cursor/rules` discovery).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceFactSnapshotItem {
    /// Redacted or hashed slugs / labels only by default.
    pub skill_slugs: Vec<String>,
    pub rule_slugs: Vec<String>,
}

/// Union of all canonical item shapes for `expand_ingest_batch` and mappers.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CanonicalItem {
    Event(EventItem),
    ToolSpan(ToolSpanItem),
    RepoSnapshotChunk(RepoSnapshotChunkItem),
    /// Populated in Phase 6; expand does not emit this from ingest batches.
    WorkspaceFactSnapshot {
        envelope: CanonicalEnvelope,
        name: CanonicalEventName,
        payload: WorkspaceFactSnapshotItem,
    },
}

/// Expand a redacted ingest batch to one struct per event/span/chunk; never drops rows.
pub fn expand_ingest_batch(batch: &crate::sync::IngestExportBatch) -> Vec<CanonicalItem> {
    use crate::sync::IngestExportBatch;
    let mut out: Vec<CanonicalItem> = Vec::new();
    match batch {
        IngestExportBatch::Events(b) => {
            let env = canonical_envelope(&b.team_id, &b.workspace_hash);
            for e in &b.events {
                out.push(CanonicalItem::Event(EventItem {
                    envelope: env.clone(),
                    name: CanonicalEventName::Event,
                    event: e.clone(),
                }));
            }
        }
        IngestExportBatch::ToolSpans(b) => {
            let env = canonical_envelope(&b.team_id, &b.workspace_hash);
            for span in &b.spans {
                out.push(CanonicalItem::ToolSpan(ToolSpanItem {
                    envelope: env.clone(),
                    name: CanonicalEventName::ToolSpan,
                    span: span.clone(),
                }));
            }
        }
        IngestExportBatch::RepoSnapshots(b) => {
            let env = canonical_envelope(&b.team_id, &b.workspace_hash);
            for chunk in &b.snapshots {
                out.push(CanonicalItem::RepoSnapshotChunk(RepoSnapshotChunkItem {
                    envelope: env.clone(),
                    name: CanonicalEventName::RepoSnapshotChunk,
                    chunk: chunk.clone(),
                }));
            }
        }
        IngestExportBatch::WorkspaceFacts(b) => {
            let env = canonical_envelope(&b.team_id, &b.workspace_hash);
            for row in &b.facts {
                out.push(CanonicalItem::WorkspaceFactSnapshot {
                    envelope: env.clone(),
                    name: CanonicalEventName::WorkspaceFactSnapshot,
                    payload: WorkspaceFactSnapshotItem {
                        skill_slugs: row.skill_slugs.clone(),
                        rule_slugs: row.rule_slugs.clone(),
                    },
                });
            }
        }
    }
    out
}

impl CanonicalItem {
    /// Short name for third-party tags / metrics (`kaizen.event`, `kaizen.tool_span`, …).
    pub fn telemetry_kind(&self) -> &'static str {
        match self {
            CanonicalItem::Event(_) => "kaizen.event",
            CanonicalItem::ToolSpan(_) => "kaizen.tool_span",
            CanonicalItem::RepoSnapshotChunk(_) => "kaizen.repo_snapshot_chunk",
            CanonicalItem::WorkspaceFactSnapshot { .. } => "kaizen.workspace_fact_snapshot",
        }
    }

    /// Schema version for assertions and exporters; workspace fact variant included.
    pub fn envelope_kaizen_schema_version(&self) -> Option<u32> {
        match self {
            CanonicalItem::Event(i) => Some(i.envelope.kaizen_schema_version),
            CanonicalItem::ToolSpan(i) => Some(i.envelope.kaizen_schema_version),
            CanonicalItem::RepoSnapshotChunk(i) => Some(i.envelope.kaizen_schema_version),
            CanonicalItem::WorkspaceFactSnapshot { envelope, .. } => {
                Some(envelope.kaizen_schema_version)
            }
        }
    }
}

fn canonical_envelope(team_id: &str, workspace_hash: &str) -> CanonicalEnvelope {
    CanonicalEnvelope {
        kaizen_schema_version: KAIZEN_SCHEMA_VERSION,
        team_id: team_id.to_string(),
        workspace_hash: workspace_hash.to_string(),
        identity: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync::IngestExportBatch;
    use crate::sync::outbound::EventsBatchBody;
    use crate::sync::smart::{OutboundToolSpan, ToolSpansBatchBody};

    #[test]
    fn expand_events_one_per_row() {
        let b = IngestExportBatch::Events(EventsBatchBody {
            team_id: "t1".into(),
            workspace_hash: "wh".into(),
            events: vec![
                OutboundEvent {
                    session_id_hash: "s1".into(),
                    event_seq: 0,
                    ts_ms: 1,
                    agent: "a".into(),
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
                },
                OutboundEvent {
                    session_id_hash: "s1".into(),
                    event_seq: 1,
                    ts_ms: 2,
                    agent: "a".into(),
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
                },
            ],
        });
        let v = expand_ingest_batch(&b);
        assert_eq!(v.len(), 2);
        assert_eq!(
            v[0].envelope_kaizen_schema_version().unwrap(),
            KAIZEN_SCHEMA_VERSION
        );
    }

    #[test]
    fn expand_tool_spans_n_items() {
        let b = IngestExportBatch::ToolSpans(ToolSpansBatchBody {
            team_id: "t".into(),
            workspace_hash: "w".into(),
            spans: vec![OutboundToolSpan {
                session_id_hash: "sh".into(),
                span_id_hash: "ph".into(),
                tool: None,
                status: "ok".into(),
                started_at_ms: None,
                ended_at_ms: None,
                lead_time_ms: None,
                tokens_in: None,
                tokens_out: None,
                reasoning_tokens: None,
                cost_usd_e6: None,
                path_hashes: vec![],
            }],
        });
        let v = expand_ingest_batch(&b);
        assert_eq!(v.len(), 1);
        assert!(matches!(v[0], CanonicalItem::ToolSpan(_)));
    }

    #[test]
    fn expand_workspace_facts_one_per_row() {
        use crate::sync::smart::{OutboundWorkspaceFactRow, WorkspaceFactsBatchBody};
        let b = IngestExportBatch::WorkspaceFacts(WorkspaceFactsBatchBody {
            team_id: "t".into(),
            workspace_hash: "w".into(),
            facts: vec![OutboundWorkspaceFactRow {
                skill_slugs: vec!["a".into()],
                rule_slugs: vec!["b".into()],
            }],
        });
        let v = expand_ingest_batch(&b);
        assert_eq!(v.len(), 1);
        assert!(matches!(v[0], CanonicalItem::WorkspaceFactSnapshot { .. }));
    }
}
