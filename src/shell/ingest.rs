// SPDX-License-Identifier: AGPL-3.0-or-later
//! `kaizen ingest` — hook ingestion (stdin or explicit payload for MCP).

use crate::collect::hooks::EventKind;
use crate::core::config;
use crate::store::Store;
use crate::{collect, core::event::SessionRecord, prompt};
use anyhow::Result;
use serde_json::Value;
use std::ffi::OsString;
use std::path::PathBuf;

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
    let event = match source {
        IngestSource::Cursor => collect::hooks::cursor::parse_cursor_hook(input)?,
        IngestSource::Claude => collect::hooks::claude::parse_claude_hook(input)?,
        IngestSource::Openclaw => collect::hooks::openclaw::parse_openclaw_hook(input)?,
        IngestSource::Vibe => collect::hooks::vibe::parse_vibe_hook(input)?,
    };
    let ws = workspace.unwrap_or_else(|| std::env::current_dir().expect("cwd"));
    let cfg = config::load(&ws)?;
    let sync_ctx = crate::sync::ingest_ctx(&cfg, ws.clone());
    let db_path = crate::core::workspace::db_path(&ws)?;
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
            let snap = prompt::snapshot::capture(&ws, now_ms).ok();
            let fingerprint = snap.as_ref().map(|s| s.fingerprint.clone());
            if let Some(ref s) = snap {
                let _ = store.upsert_prompt_snapshot(s);
            }
            let model = collect::model_from_json::from_value(&event.payload);
            let env = session_env_fields(&event.payload);
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
                source.agent(),
                &ws.to_string_lossy(),
                event.ts_ms,
            )?;
            if matches!(event.kind, collect::hooks::EventKind::Stop) {
                maybe_emit_prompt_changed(
                    &store,
                    &event.session_id,
                    &ws,
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
            source.agent(),
            &ws.to_string_lossy(),
            event.ts_ms,
        )?;
    }
    store.append_event_with_sync(&ev, sync_ctx.as_ref())?;
    post_ingest_detached(&event, &cfg, &ws)?;
    Ok(())
}

/// Non-blocking sidecars: outcome worker, sampler child, stop file (hooks stay short).
fn post_ingest_detached(
    event: &collect::hooks::HookEvent,
    cfg: &config::Config,
    ws: &std::path::Path,
) -> Result<()> {
    if matches!(event.kind, EventKind::Stop) {
        if cfg.collect.outcomes.enabled {
            spawn_outcome_measure(ws, &event.session_id);
        }
        if cfg.collect.system_sampler.enabled {
            touch_sampler_stop_file(ws, &event.session_id);
        }
    }
    if matches!(event.kind, EventKind::SessionStart)
        && cfg.collect.system_sampler.enabled
        && let Some(pid) = payload_pid(&event.payload)
    {
        spawn_sampler_run(ws, &event.session_id, pid);
    }
    Ok(())
}

fn payload_pid(v: &Value) -> Option<u32> {
    v.get("pid")
        .and_then(|x| x.as_u64().map(|n| n as u32))
        .or_else(|| {
            v.get("pid")
                .and_then(|x| x.as_i64())
                .and_then(|i| u32::try_from(i).ok())
        })
}

fn spawn_outcome_measure(ws: &std::path::Path, session_id: &str) {
    let args = vec![
        OsString::from("outcomes"),
        OsString::from("measure"),
        OsString::from("--workspace"),
        ws.as_os_str().to_owned(),
        OsString::from("--session"),
        OsString::from(session_id),
    ];
    if let Err(e) = super::kaizen_child::spawn_kaizen_detached(&args) {
        tracing::warn!(?e, "kaizen outcomes measure");
    }
}

fn spawn_sampler_run(ws: &std::path::Path, session_id: &str, pid: u32) {
    let args = vec![
        OsString::from("__sampler-run"),
        OsString::from("--workspace"),
        ws.as_os_str().to_owned(),
        OsString::from("--session"),
        OsString::from(session_id),
        OsString::from("--pid"),
        OsString::from(pid.to_string()),
    ];
    if let Err(e) = super::kaizen_child::spawn_kaizen_detached(&args) {
        tracing::warn!(?e, "kaizen sampler");
    }
}

fn touch_sampler_stop_file(ws: &std::path::Path, session_id: &str) {
    let dir = match crate::core::paths::project_data_dir(ws) {
        Ok(d) => d.join("sampler-stop"),
        Err(e) => {
            tracing::warn!(?e, "sampler-stop: no data dir");
            return;
        }
    };
    if let Err(e) = std::fs::create_dir_all(&dir) {
        tracing::warn!(?e, "sampler-stop mkdir");
        return;
    }
    let path = dir.join(session_id);
    if let Err(e) = std::fs::File::create(&path) {
        tracing::warn!(?e, "sampler-stop touch");
    }
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

fn maybe_emit_prompt_changed(
    store: &Store,
    session_id: &str,
    ws: &std::path::Path,
    now_ms: u64,
    trigger_ev: &crate::core::event::Event,
    sync_ctx: Option<&crate::sync::context::SyncIngestContext>,
) -> Result<()> {
    let Some(session) = store.get_session(session_id)? else {
        return Ok(());
    };
    let Some(from_fp) = session.prompt_fingerprint else {
        return Ok(());
    };
    let snap = prompt::snapshot::capture(ws, now_ms).ok();
    let Some(snap) = snap else { return Ok(()) };
    if snap.fingerprint == from_fp {
        return Ok(());
    }
    let _ = store.upsert_prompt_snapshot(&snap);
    let changed_ev = crate::core::event::Event {
        session_id: session_id.to_string(),
        seq: trigger_ev.seq + 1,
        ts_ms: now_ms,
        ts_exact: true,
        kind: crate::core::event::EventKind::Hook,
        source: crate::core::event::EventSource::Hook,
        tool: None,
        tool_call_id: None,
        tokens_in: None,
        tokens_out: None,
        reasoning_tokens: None,
        cost_usd_e6: None,
        stop_reason: None,
        latency_ms: None,
        ttft_ms: None,
        retry_count: None,
        context_used_tokens: None,
        context_max_tokens: None,
        cache_creation_tokens: None,
        cache_read_tokens: None,
        system_prompt_tokens: None,
        payload: serde_json::json!({
            "kind": "prompt_changed",
            "from_fingerprint": from_fp,
            "to_fingerprint": snap.fingerprint,
        }),
    };
    store.append_event_with_sync(&changed_ev, sync_ctx)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::paths::test_lock;
    use tempfile::TempDir;

    fn setup_ws() -> (TempDir, TempDir) {
        let home = TempDir::new().unwrap();
        let ws = TempDir::new().unwrap();
        unsafe { std::env::set_var("KAIZEN_HOME", home.path()) };
        (home, ws)
    }

    #[test]
    fn session_start_records_source_as_agent_not_unknown() {
        let _guard = test_lock::global().lock().unwrap();
        let (_home, ws) = setup_ws();
        let payload =
            r#"{"hook_event_name":"SessionStart","session_id":"s-agent-1","source":"startup"}"#;
        ingest_hook_text(IngestSource::Claude, payload, Some(ws.path().to_path_buf())).unwrap();
        let db = Store::open(&crate::core::workspace::db_path(ws.path()).unwrap()).unwrap();
        let sessions = db
            .list_sessions(ws.path().to_string_lossy().as_ref())
            .unwrap();
        unsafe { std::env::remove_var("KAIZEN_HOME") };
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].agent, "claude");
    }

    #[test]
    fn missing_timestamp_falls_back_to_now() {
        let _guard = test_lock::global().lock().unwrap();
        let (_home, ws) = setup_ws();
        let payload =
            r#"{"hook_event_name":"SessionStart","session_id":"s-ts","source":"startup"}"#;
        ingest_hook_text(IngestSource::Claude, payload, Some(ws.path().to_path_buf())).unwrap();
        let db = Store::open(&crate::core::workspace::db_path(ws.path()).unwrap()).unwrap();
        let sessions = db
            .list_sessions(ws.path().to_string_lossy().as_ref())
            .unwrap();
        unsafe { std::env::remove_var("KAIZEN_HOME") };
        assert!(sessions[0].started_at_ms > 0, "started_at_ms must not be 0");
    }

    #[test]
    fn post_tool_use_without_session_start_auto_provisions_stub() {
        let _guard = test_lock::global().lock().unwrap();
        let (_home, ws) = setup_ws();
        let payload = r#"{"event":"PostToolUse","session_id":"s-stub","tool_name":"Read","tool_input":{"file_path":"/tmp/x"},"tool_response":{"content":"hi"}}"#;
        ingest_hook_text(IngestSource::Cursor, payload, Some(ws.path().to_path_buf())).unwrap();
        let db = Store::open(&crate::core::workspace::db_path(ws.path()).unwrap()).unwrap();
        let sessions = db
            .list_sessions(ws.path().to_string_lossy().as_ref())
            .unwrap();
        unsafe { std::env::remove_var("KAIZEN_HOME") };
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].agent, "cursor");
        assert_eq!(sessions[0].id, "s-stub");
    }
}
