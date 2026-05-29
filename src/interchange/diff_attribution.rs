// SPDX-License-Identifier: AGPL-3.0-or-later
//! Diff attribution DTOs. Raw patches stay opt-in.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffAttributionOptions {
    pub include_raw_patch: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffAttribution {
    pub session_id: String,
    pub commit: String,
    pub summary: DiffSummary,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<FileDiffAttribution>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_patch: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffSummary {
    pub files_changed: u32,
    pub additions: u32,
    pub deletions: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileDiffAttribution {
    pub path: String,
    pub change: DiffChangeKind,
    pub additions: u32,
    pub deletions: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hunks: Vec<DiffHunkAttribution>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_patch: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffChangeKind {
    Added,
    Modified,
    Deleted,
    Renamed { previous_path: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffHunkAttribution {
    pub old_start: u32,
    pub new_start: u32,
    pub old_lines: u32,
    pub new_lines: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_fingerprint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_patch: Option<String>,
}

impl DiffAttributionOptions {
    pub fn include_raw_patch() -> Self {
        Self {
            include_raw_patch: true,
        }
    }
}

impl DiffAttribution {
    pub fn new(session_id: impl Into<String>, commit: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            commit: commit.into(),
            summary: DiffSummary::default(),
            files: Vec::new(),
            raw_patch: None,
        }
    }

    pub fn with_raw_patch(
        mut self,
        raw_patch: impl Into<String>,
        options: DiffAttributionOptions,
    ) -> Self {
        if options.include_raw_patch {
            self.raw_patch = Some(raw_patch.into());
        }
        self
    }
}
