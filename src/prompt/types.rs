// SPDX-License-Identifier: AGPL-3.0-or-later
//! Pure data types for prompt snapshot tracking.

use serde::{Deserialize, Serialize};

/// One file that contributes to the prompt fingerprint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptFile {
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
}

/// Immutable snapshot of all prompt/rule/skill files at a point in time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptSnapshot {
    /// Blake3 hash over sorted file contents — primary key in the store.
    pub fingerprint: String,
    pub captured_at_ms: u64,
    /// JSON-serialised `Vec<PromptFile>`.
    pub files_json: String,
    pub total_bytes: u64,
}

impl PromptSnapshot {
    pub fn files(&self) -> Vec<PromptFile> {
        serde_json::from_str(&self.files_json).unwrap_or_default()
    }
}

/// Difference between two snapshots.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PromptDiff {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub changed: Vec<String>,
}

impl PromptDiff {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.changed.is_empty()
    }
}
