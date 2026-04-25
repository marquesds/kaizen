// SPDX-License-Identifier: AGPL-3.0-or-later
//! SQLite: ensure proxy session, append one `Cost` or `Error` per completed forward.

use crate::core::config::Config;
use crate::core::cost::CostTable;
use crate::core::event::{Event, EventKind, EventSource, SessionRecord, SessionStatus};
use crate::store::Store;
use crate::sync::ingest_ctx;
use anyhow::Context;
use serde_json::json;
use std::path::Path;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

static COST_TABLE: OnceLock<CostTable> = OnceLock::new();

fn bundled_cost_table() -> &'static CostTable {
    COST_TABLE.get_or_init(|| CostTable::load().expect("bundled assets/cost.toml"))
}

/// Per ADR 002: estimate from price table. Skip heuristic for failed forwards with no usage
/// (avoids attributing fake spend to transport errors).
///
/// Successful responses with no `usage` in the body use the bundled `cursor` row heuristic,
/// same as when the model id is unknown — avoids `cost_usd_e6 = 0` for priced models with
/// zero tokens.
fn proxy_event_cost_usd_e6(a: &RecordArgs) -> Option<i64> {
    let saw_usage = a.tokens_in.is_some() || a.tokens_out.is_some() || a.reasoning_tokens.is_some();
    if a.upstream_error.is_some() && !saw_usage {
        return None;
    }
    let tin = a.tokens_in.unwrap_or(0);
    let tout = a
        .tokens_out
        .unwrap_or(0)
        .saturating_add(a.reasoning_tokens.unwrap_or(0));
    let table = bundled_cost_table();
    let cost = if a.upstream_error.is_none() && !saw_usage {
        table.estimate(None, 0, 0)
    } else {
        table.estimate(a.model.as_deref(), tin, tout)
    };
    Some(cost)
}

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
            prompt_fingerprint: None,
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
        cost_usd_e6: proxy_event_cost_usd_e6(a),
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

    fn sample_args() -> RecordArgs {
        RecordArgs {
            session_id: "s".into(),
            model: Some("claude-sonnet-4".into()),
            path: "/v1/messages".into(),
            method: "POST".into(),
            status: 200,
            request_id: None,
            tokens_in: None,
            tokens_out: None,
            reasoning_tokens: None,
            upstream_error: None,
        }
    }

    #[test]
    fn time_ok() {
        assert!(now_ms().unwrap() > 1_000_000_000);
    }

    #[test]
    fn proxy_cost_matches_table_when_tokens_present() {
        let mut a = sample_args();
        a.tokens_in = Some(1000);
        a.tokens_out = Some(500);
        assert_eq!(proxy_event_cost_usd_e6(&a), Some(10_500));
    }

    #[test]
    fn proxy_cost_heuristic_when_success_but_no_usage() {
        let a = sample_args();
        assert!(proxy_event_cost_usd_e6(&a).is_some_and(|c| c > 0));
    }

    #[test]
    fn proxy_cost_none_on_error_without_usage() {
        let mut a = sample_args();
        a.upstream_error = Some("timeout".into());
        assert_eq!(proxy_event_cost_usd_e6(&a), None);
    }

    #[test]
    fn proxy_cost_on_error_when_usage_present() {
        let mut a = sample_args();
        a.upstream_error = Some("upstream 429".into());
        a.tokens_in = Some(100);
        a.tokens_out = Some(50);
        assert!(proxy_event_cost_usd_e6(&a).is_some_and(|c| c > 0));
    }
}
