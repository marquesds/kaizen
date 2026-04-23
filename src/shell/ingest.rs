// SPDX-License-Identifier: AGPL-3.0-or-later
//! `kaizen ingest` — hook ingestion (stdin or explicit payload for MCP).

use crate::core::config;
use crate::store::Store;
use crate::{collect, core::event::SessionRecord};
use anyhow::Result;
use std::path::PathBuf;

/// Hook source, aligned with the `kaizen ingest hook --source` CLI.
#[derive(Clone, Copy, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum IngestSource {
    Cursor,
    Claude,
}

impl IngestSource {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "cursor" => Some(Self::Cursor),
            "claude" => Some(Self::Claude),
            _ => None,
        }
    }
}

/// Process hook JSON (same as stdin for `kaizen ingest hook`). On success, returns empty string (CLI prints nothing).
pub fn ingest_hook_string(
    source: IngestSource,
    input: &str,
    workspace: Option<PathBuf>,
) -> Result<String> {
    ingest_hook_text(source, input, workspace)?;
    Ok(String::new())
}

/// Process hook JSON (same as stdin for `kaizen ingest hook`).
pub fn ingest_hook_text(
    source: IngestSource,
    input: &str,
    workspace: Option<PathBuf>,
) -> Result<()> {
    let event = match source {
        IngestSource::Cursor => collect::hooks::cursor::parse_cursor_hook(input)?,
        IngestSource::Claude => collect::hooks::claude::parse_claude_hook(input)?,
    };
    let ws = workspace.unwrap_or_else(|| std::env::current_dir().expect("cwd"));
    let cfg = config::load(&ws)?;
    let sync_ctx = crate::sync::ingest_ctx(&cfg, ws.clone());
    let db_path = ws.join(".kaizen/kaizen.db");
    let store = Store::open(&db_path)?;
    let ev = collect::hooks::normalize::hook_to_event(&event, 0);
    if let Some(status) = collect::hooks::normalize::hook_to_status(&event.kind) {
        if matches!(event.kind, collect::hooks::EventKind::SessionStart) {
            let model = collect::model_from_json::from_value(&event.payload);
            let record = SessionRecord {
                id: event.session_id.clone(),
                agent: "unknown".to_string(),
                model,
                workspace: ws.to_string_lossy().to_string(),
                started_at_ms: event.ts_ms,
                ended_at_ms: None,
                status: status.clone(),
                trace_path: String::new(),
                start_commit: None,
                end_commit: None,
                branch: None,
                dirty_start: None,
                dirty_end: None,
                repo_binding_source: None,
            };
            store.upsert_session(&record)?;
        } else {
            store.update_session_status(&event.session_id, status)?;
        }
    }
    store.append_event_with_sync(&ev, sync_ctx.as_ref())?;
    Ok(())
}
