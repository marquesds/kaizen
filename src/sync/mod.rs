// SPDX-License-Identifier: AGPL-3.0-or-later
//! Sync daemon: redacted outbox → `POST /v1/events`.

pub mod canonical;
pub mod client;
pub mod context;
pub mod engine;
pub mod export_batch;
pub mod outbound;
pub mod redact;
pub mod smart;
pub mod telemetry_replay;

pub use canonical::{
    CanonicalEnvelope, CanonicalEventName, CanonicalItem, EventItem, KAIZEN_SCHEMA_VERSION,
    RepoSnapshotChunkItem, ToolSpanItem, WorkspaceFactSnapshotItem, expand_ingest_batch,
};
pub use context::SyncIngestContext;
pub use engine::{FlushExporters, FlushStats, flush_outbox_once};
pub use export_batch::IngestExportBatch;
pub use outbound::{EventsBatchBody, OutboundEvent, hash_with_salt, workspace_hash};
pub use telemetry_replay::{
    chunk_events_into_ingest_batches, chunk_tool_spans_into_ingest_batches,
};

use crate::core::config::Config;
use std::path::PathBuf;

/// When sync endpoint is configured, pass this into `append_event_with_sync`.
pub fn ingest_ctx(cfg: &Config, workspace_root: PathBuf) -> Option<SyncIngestContext> {
    if cfg.sync.endpoint.is_empty() {
        return None;
    }
    Some(SyncIngestContext::new(cfg.sync.clone(), workspace_root))
}
