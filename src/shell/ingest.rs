// SPDX-License-Identifier: AGPL-3.0-or-later
//! `kaizen ingest` — hook ingestion (stdin or explicit payload for MCP).

use crate::core::config;
use crate::store::Store;
use crate::{collect, core::event::SessionRecord, prompt};
use anyhow::Result;
use serde_json::Value;
use std::path::PathBuf;

mod identity;
mod prompt_change;
mod sidecars;
#[cfg(test)]
mod tests;

use prompt_change::maybe_emit_prompt_changed;
use sidecars::post_ingest_detached;

/// Hook source, aligned with the `kaizen ingest hook --source` CLI.
#[derive(Clone, Copy, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum IngestSource {
    Cursor,
    Claude,
    Openclaw,
    Vibe,
}

impl IngestSource {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "cursor" => Some(Self::Cursor),
            "claude" => Some(Self::Claude),
            "openclaw" => Some(Self::Openclaw),
            "vibe" => Some(Self::Vibe),
            _ => None,
        }
    }

    pub fn agent(self) -> &'static str {
        match self {
            Self::Cursor => "cursor",
            Self::Claude => "claude",
            Self::Openclaw => "openclaw",
            Self::Vibe => "vibe",
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
    let ws = workspace.unwrap_or_else(|| std::env::current_dir().expect("cwd"));
    let db_path = crate::core::workspace::db_path(&ws)?;
    let store = Store::open(&db_path)?;
    ingest_hook_with_store(source, input, &ws, &store)
}

pub(crate) fn ingest_hook_with_store(
    source: IngestSource,
    input: &str,
    ws: &std::path::Path,
    store: &Store,
) -> Result<()> {
    let event = match source {
        IngestSource::Cursor => collect::hooks::cursor::parse_cursor_hook(input)?,
        IngestSource::Claude => collect::hooks::claude::parse_claude_hook(input)?,
        IngestSource::Openclaw => collect::hooks::openclaw::parse_openclaw_hook(input)?,
        IngestSource::Vibe => collect::hooks::vibe::parse_vibe_hook(input)?,
    };
    let cfg = config::load(ws)?;
    let sync_ctx = crate::sync::ingest_ctx(&cfg, ws.to_path_buf());
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let ts = if event.ts_ms == 0 {
        now_ms
    } else {
        event.ts_ms
    };
    let mut event = event;
    event.ts_ms = ts;
    let identity = identity::from_payload(source, &event.payload);
    let seq = store.next_event_seq(&event.session_id)?;
    let ev = collect::hooks::normalize::hook_to_event(&event, seq);
    if let Some(status) = collect::hooks::normalize::hook_to_status(&event.kind) {
        if matches!(event.kind, collect::hooks::EventKind::SessionStart) {
            let snap = prompt::snapshot::capture(ws, now_ms).ok();
            let fingerprint = snap.as_ref().map(|s| s.fingerprint.clone());
            if let Some(ref s) = snap {
                let _ = store.upsert_prompt_snapshot(s);
            }
            let model = collect::model_from_json::from_value(&event.payload);
            let env = session_env_fields(&event.payload);
            let record = SessionRecord {
                id: event.session_id.clone(),
                agent: identity.agent.clone(),
                model: identity.model.clone().or(model),
                workspace: ws.to_string_lossy().to_string(),
                started_at_ms: event.ts_ms,
                ended_at_ms: None,
                status: status.clone(),
                trace_path: identity.trace_path.clone().unwrap_or_default(),
                start_commit: None,
                end_commit: None,
                branch: None,
                dirty_start: None,
                dirty_end: None,
                repo_binding_source: None,
                prompt_fingerprint: fingerprint,
                parent_session_id: None,
                agent_version: env.0,
                os: env.1,
                arch: env.2,
                repo_file_count: None,
                repo_total_loc: None,
            };
            store.upsert_session(&record)?;
        } else {
            store.ensure_session_stub(
                &event.session_id,
                &identity.agent,
                &ws.to_string_lossy(),
                event.ts_ms,
            )?;
            if matches!(event.kind, collect::hooks::EventKind::Stop) {
                maybe_emit_prompt_changed(
                    store,
                    &event.session_id,
                    ws,
                    now_ms,
                    &ev,
                    sync_ctx.as_ref(),
                )?;
            }
            store.update_session_status(&event.session_id, status)?;
        }
    } else {
        store.ensure_session_stub(
            &event.session_id,
            &identity.agent,
            &ws.to_string_lossy(),
            event.ts_ms,
        )?;
    }
    store.enrich_session_identity(
        &event.session_id,
        identity.agent_update.as_deref(),
        identity.model.as_deref(),
        identity.trace_path.as_deref(),
    )?;
    store.append_event_with_sync(&ev, sync_ctx.as_ref())?;
    if matches!(event.kind, collect::hooks::EventKind::Stop) {
        store.flush_search()?;
    }
    post_ingest_detached(&event, &cfg, ws)?;
    Ok(())
}

fn session_env_fields(payload: &Value) -> (Option<String>, Option<String>, Option<String>) {
    let ver = [
        "cursor_version",
        "claude_version",
        "agent_version",
        "version",
    ]
    .into_iter()
    .find_map(|k| {
        payload
            .get(k)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    });
    let os = payload
        .get("os")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let arch = payload
        .get("arch")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    (ver, os, arch)
}
