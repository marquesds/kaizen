//! Sync daemon: redacted outbox → `POST /v1/events`.

pub mod client;
pub mod context;
pub mod engine;
pub mod outbound;
pub mod redact;

pub use context::SyncIngestContext;
pub use engine::{flush_outbox_once, FlushStats};
pub use outbound::{hash_with_salt, workspace_hash, EventsBatchBody, OutboundEvent};

use crate::core::config::Config;
use std::path::PathBuf;

/// When sync endpoint is configured, pass this into `append_event_with_sync`.
pub fn ingest_ctx(cfg: &Config, workspace_root: PathBuf) -> Option<SyncIngestContext> {
    if cfg.sync.endpoint.is_empty() {
        return None;
    }
    Some(SyncIngestContext::new(cfg.sync.clone(), workspace_root))
}
