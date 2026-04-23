//! Sync daemon: redacted outbox → `POST /v1/events`.

pub mod client;
pub mod context;
pub mod engine;
pub mod outbound;
pub mod redact;
pub mod smart;

pub use context::SyncIngestContext;
pub use engine::{FlushStats, flush_outbox_once};
pub use outbound::{EventsBatchBody, OutboundEvent, hash_with_salt, workspace_hash};

use crate::core::config::Config;
use std::path::PathBuf;

/// When sync endpoint is configured, pass this into `append_event_with_sync`.
pub fn ingest_ctx(cfg: &Config, workspace_root: PathBuf) -> Option<SyncIngestContext> {
    if cfg.sync.endpoint.is_empty() {
        return None;
    }
    Some(SyncIngestContext::new(cfg.sync.clone(), workspace_root))
}
