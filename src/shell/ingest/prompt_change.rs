// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::prompt;
use crate::store::Store;
use anyhow::Result;
use std::path::Path;

pub(super) fn maybe_emit_prompt_changed(
    store: &Store,
    session_id: &str,
    workspace: &Path,
    now_ms: u64,
    trigger: &crate::core::event::Event,
    sync: Option<&crate::sync::context::SyncIngestContext>,
) -> Result<()> {
    let Some(session) = store.get_session(session_id)? else {
        return Ok(());
    };
    let Some(from) = session.prompt_fingerprint else {
        return Ok(());
    };
    let Some(snapshot) = prompt::snapshot::capture(workspace, now_ms).ok() else {
        return Ok(());
    };
    if snapshot.fingerprint == from {
        return Ok(());
    }
    store.upsert_prompt_snapshot(&snapshot)?;
    store.append_event_with_sync(
        &changed_event(session_id, now_ms, trigger, from, snapshot.fingerprint),
        sync,
    )
}

fn changed_event(
    session_id: &str,
    now_ms: u64,
    trigger: &crate::core::event::Event,
    from: String,
    to: String,
) -> crate::core::event::Event {
    crate::core::event::Event {
        session_id: session_id.to_owned(),
        seq: trigger.seq + 1,
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
            "from_fingerprint": from,
            "to_fingerprint": to,
        }),
    }
}
