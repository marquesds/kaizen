// SPDX-License-Identifier: AGPL-3.0-or-later
//! SQLite: ensure proxy session, append one `Cost` or `Error` per completed forward.

use crate::core::config::Config;
use crate::core::event::{Event, EventKind, EventSource, SessionRecord, SessionStatus};
use crate::store::Store;
use crate::sync::ingest_ctx;
use anyhow::Context;
use serde_json::json;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Append telemetry for one upstream round-trip. Pure sync — call from `spawn_blocking`.
pub fn record_forward_outcome(
    store_path: &Path,
    cfg: &Config,
    workspace: &Path,
    a: &RecordArgs,
) -> Result<(), anyhow::Error> {
    let store = Store::open(store_path).context("open kaizen store")?;
    let sync_c = ingest_ctx(cfg, workspace.to_path_buf());
    let now = now_ms()?;
    if store.get_session(&a.session_id)?.is_none() {
        let rec = SessionRecord {
            id: a.session_id.clone(),
            agent: "claude".to_string(),
            model: a.model.clone(),
            workspace: workspace.to_string_lossy().into_owned(),
            started_at_ms: now,
            ended_at_ms: None,
            status: SessionStatus::Running,
            trace_path: String::new(),
            start_commit: None,
            end_commit: None,
            branch: None,
            dirty_start: None,
            dirty_end: None,
            repo_binding_source: None,
        };
        store.upsert_session(&rec)?;
    }
    let seq = store.next_event_seq(&a.session_id)?;
    let (kind, payload) = if let Some(ref err) = a.upstream_error {
        (
            EventKind::Error,
            json!({
                "path": a.path,
                "method": a.method,
                "status": a.status,
                "upstream_error": err,
            }),
        )
    } else {
        let mut p = json!({
            "path": a.path,
            "method": a.method,
            "status": a.status,
            "model": a.model,
        });
        if let Some(rid) = &a.request_id {
            p["request_id"] = json!(rid);
        }
        (EventKind::Cost, p)
    };
    let e = Event {
        session_id: a.session_id.clone(),
        seq,
        ts_ms: now,
        ts_exact: true,
        kind,
        source: EventSource::Proxy,
        tool: None,
        tool_call_id: None,
        tokens_in: a.tokens_in,
        tokens_out: a.tokens_out,
        reasoning_tokens: a.reasoning_tokens,
        cost_usd_e6: None,
        payload,
    };
    store.append_event_with_sync(&e, sync_c.as_ref())?;
    Ok(())
}

#[derive(Clone)]
pub struct RecordArgs {
    pub session_id: String,
    pub model: Option<String>,
    pub path: String,
    pub method: String,
    pub status: u16,
    pub request_id: Option<String>,
    pub tokens_in: Option<u32>,
    pub tokens_out: Option<u32>,
    pub reasoning_tokens: Option<u32>,
    pub upstream_error: Option<String>,
}

fn now_ms() -> Result<u64, anyhow::Error> {
    let d = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(d.as_millis() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_ok() {
        assert!(now_ms().unwrap() > 1_000_000_000);
    }
}
