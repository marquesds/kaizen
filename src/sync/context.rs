// SPDX-License-Identifier: AGPL-3.0-or-later
//! Context passed when appending events so the store can enqueue the sync outbox.

use crate::core::config::SyncConfig;
use std::path::{Path, PathBuf};

/// Everything needed to optionally enqueue a redacted row after a successful insert.
#[derive(Debug, Clone)]
pub struct SyncIngestContext {
    pub sync: SyncConfig,
    pub workspace_root: PathBuf,
}

impl SyncIngestContext {
    pub fn new(sync: SyncConfig, workspace_root: PathBuf) -> Self {
        Self {
            sync,
            workspace_root,
        }
    }

    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }
}
