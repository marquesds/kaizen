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
    Openclaw,
}

impl IngestSource {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "cursor" => Some(Self::Cursor),
            "claude" => Some(Self::Claude),
            "openclaw" => Some(Self::Openclaw),
            _ => None,
        }
    }

    pub fn agent(self) -> &'static str {
        match self {
            Self::Cursor => "cursor",
            Self::Claude => "claude",
            Self::Openclaw => "openclaw",
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
        IngestSource::Openclaw => collect::hooks::openclaw::parse_openclaw_hook(input)?,
    };
    let ws = workspace.unwrap_or_else(|| std::env::current_dir().expect("cwd"));
    let cfg = config::load(&ws)?;
    let sync_ctx = crate::sync::ingest_ctx(&cfg, ws.clone());
    let db_path = ws.join(".kaizen/kaizen.db");
    let store = Store::open(&db_path)?;
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
    let ev = collect::hooks::normalize::hook_to_event(&event, 0);
    if let Some(status) = collect::hooks::normalize::hook_to_status(&event.kind) {
        if matches!(event.kind, collect::hooks::EventKind::SessionStart) {
            let model = collect::model_from_json::from_value(&event.payload);
            let record = SessionRecord {
                id: event.session_id.clone(),
                agent: source.agent().to_string(),
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
            store.ensure_session_stub(
                &event.session_id,
                source.agent(),
                &ws.to_string_lossy(),
                event.ts_ms,
            )?;
            store.update_session_status(&event.session_id, status)?;
        }
    } else {
        store.ensure_session_stub(
            &event.session_id,
            source.agent(),
            &ws.to_string_lossy(),
            event.ts_ms,
        )?;
    }
    store.append_event_with_sync(&ev, sync_ctx.as_ref())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn ws_with_kaizen_dir() -> TempDir {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".kaizen")).unwrap();
        dir
    }

    #[test]
    fn session_start_records_source_as_agent_not_unknown() {
        let dir = ws_with_kaizen_dir();
        let payload =
            r#"{"hook_event_name":"SessionStart","session_id":"s-agent-1","source":"startup"}"#;
        ingest_hook_text(
            IngestSource::Claude,
            payload,
            Some(dir.path().to_path_buf()),
        )
        .unwrap();

        let db = Store::open(&dir.path().join(".kaizen/kaizen.db")).unwrap();
        let sessions = db
            .list_sessions(dir.path().to_string_lossy().as_ref())
            .unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].agent, "claude");
    }

    #[test]
    fn missing_timestamp_falls_back_to_now() {
        let dir = ws_with_kaizen_dir();
        // No timestamp_ms field — Claude Code never sends one.
        let payload =
            r#"{"hook_event_name":"SessionStart","session_id":"s-ts","source":"startup"}"#;
        ingest_hook_text(
            IngestSource::Claude,
            payload,
            Some(dir.path().to_path_buf()),
        )
        .unwrap();

        let db = Store::open(&dir.path().join(".kaizen/kaizen.db")).unwrap();
        let sessions = db
            .list_sessions(dir.path().to_string_lossy().as_ref())
            .unwrap();
        assert!(sessions[0].started_at_ms > 0, "started_at_ms must not be 0");
    }

    #[test]
    fn post_tool_use_without_session_start_auto_provisions_stub() {
        let dir = ws_with_kaizen_dir();
        // Hooks installed mid-session: first event is PostToolUse, no SessionStart.
        let payload = r#"{"event":"PostToolUse","session_id":"s-stub","tool_name":"Read","tool_input":{"file_path":"/tmp/x"},"tool_response":{"content":"hi"}}"#;
        ingest_hook_text(
            IngestSource::Cursor,
            payload,
            Some(dir.path().to_path_buf()),
        )
        .unwrap();

        let db = Store::open(&dir.path().join(".kaizen/kaizen.db")).unwrap();
        let sessions = db
            .list_sessions(dir.path().to_string_lossy().as_ref())
            .unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].agent, "cursor");
        assert_eq!(sessions[0].id, "s-stub");
    }
}
