// SPDX-License-Identifier: AGPL-3.0-or-later
//! WebSocket snapshot adapter for the read-only visualization screen.

use crate::core::workspace;
use crate::store::Store;
use crate::visualization::{VisualizationQuery, VisualizationReport, build_report};
use anyhow::{Result, ensure};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct SnapshotRequest {
    pub workspace: String,
    pub selected_session_id: Option<String>,
}

pub fn load(req: SnapshotRequest) -> Result<VisualizationReport> {
    ensure!(!req.workspace.trim().is_empty(), "workspace required");
    let root = workspace::canonical(&PathBuf::from(req.workspace.trim()));
    let store = Store::open(&workspace::db_path(&root)?)?;
    build_report(&store, query(req.selected_session_id, root))
}

fn query(selected_session_id: Option<String>, root: PathBuf) -> VisualizationQuery {
    VisualizationQuery {
        workspace: root.to_string_lossy().into_owned(),
        selected_session_id,
        now_ms: now_ms(),
        day_start_hour: 7,
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or_default()
}
