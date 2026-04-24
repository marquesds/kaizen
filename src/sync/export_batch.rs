// SPDX-License-Identifier: AGPL-3.0-or-later
//! Redacted batch payload shared by Kaizen sync POST and pluggable exporter fan-out.

use crate::sync::outbound::EventsBatchBody;
use crate::sync::smart::{RepoSnapshotsBatchBody, ToolSpansBatchBody, WorkspaceFactsBatchBody};

/// Same JSON bodies as the ingest API; used for both primary sync and optional exporters.
#[derive(Debug, Clone)]
pub enum IngestExportBatch {
    Events(EventsBatchBody),
    ToolSpans(ToolSpansBatchBody),
    RepoSnapshots(RepoSnapshotsBatchBody),
    WorkspaceFacts(WorkspaceFactsBatchBody),
}

impl IngestExportBatch {
    pub fn kind_name(&self) -> &'static str {
        match self {
            IngestExportBatch::Events(_) => "events",
            IngestExportBatch::ToolSpans(_) => "tool_spans",
            IngestExportBatch::RepoSnapshots(_) => "repo_snapshots",
            IngestExportBatch::WorkspaceFacts(_) => "workspace_facts",
        }
    }

    pub fn item_count(&self) -> usize {
        match self {
            IngestExportBatch::Events(b) => b.events.len(),
            IngestExportBatch::ToolSpans(b) => b.spans.len(),
            IngestExportBatch::RepoSnapshots(b) => b.snapshots.len(),
            IngestExportBatch::WorkspaceFacts(b) => b.facts.len(),
        }
    }
}
