// SPDX-License-Identifier: AGPL-3.0-or-later
//! Pure JSON builder + chunker for Datadog Logs API. No I/O here; HTTP lives in
//! [`super::transport`]. Boundary call to `gethostname` is the one exception so callers do not
//! have to thread a hostname through every test.

use crate::sync::IngestExportBatch;
use crate::sync::canonical::{CanonicalItem, expand_ingest_batch};
use serde_json::{Value, json};

/// DD Logs API caps (`POST /api/v2/logs`): up to 1000 entries and ~5 MB JSON per request.
/// We chunk a touch under 5 MB to leave room for HTTP envelope.
pub const MAX_ITEMS_PER_CHUNK: usize = 1000;
pub const MAX_BYTES_PER_CHUNK: usize = 5 * 1024 * 1024 - 64 * 1024;

/// Resolve the local hostname once; safe boundary call. Empty string if the OS denies it
/// (rare); the rest of the pipeline still emits a usable record.
pub fn current_hostname() -> String {
    gethostname::gethostname().to_string_lossy().into_owned()
}

/// Build one DD log object per canonical item. Pure and total: every input shape produces a
/// well-formed object with `timestamp` (ms) and `hostname` so DD time + facets work.
pub fn build_log_objects(batch: &IngestExportBatch, hostname: &str) -> Vec<Value> {
    expand_ingest_batch(batch)
        .iter()
        .map(|c| dd_log_object(c, hostname))
        .collect()
}

/// One Datadog log object for a single [`CanonicalItem`]. Top-level fields drive DD facets;
/// the full canonical payload is nested under `kaizen` for users who want raw detail.
pub fn dd_log_object(item: &CanonicalItem, hostname: &str) -> Value {
    let kind_tag = item.telemetry_kind();
    let ts_ms = item_timestamp_ms(item);
    let (agent, model) = item_agent_model(item);
    let mut obj = json!({
        "timestamp": ts_ms,
        "hostname": hostname,
        "ddsource": "kaizen",
        "service": "kaizen",
        "ddtags": ddtags(kind_tag, agent.as_deref(), model.as_deref()),
        "kaizen_type": kind_tag,
        "message": short_message(item, agent.as_deref(), model.as_deref()),
        "kaizen": serde_json::to_value(item).unwrap_or(Value::Null),
    });
    let m = obj.as_object_mut().expect("obj is map");
    promote_top_level(m, item);
    if let Some(a) = agent {
        m.insert("agent".to_string(), Value::String(a));
    }
    if let Some(mdl) = model {
        m.insert("model".to_string(), Value::String(mdl));
    }
    obj
}

/// Split log objects into chunks that respect both the entry-count and byte caps. Pure: the
/// sum of all chunk lengths equals the input length, order preserved.
pub fn chunk_for_dd(items: Vec<Value>) -> Vec<Vec<Value>> {
    let mut out: Vec<Vec<Value>> = Vec::new();
    let mut cur: Vec<Value> = Vec::new();
    let mut cur_bytes: usize = 2; // `[` + `]`
    for v in items {
        let bytes = serde_json::to_vec(&v).map(|b| b.len() + 1).unwrap_or(0);
        let would_overflow_items = cur.len() >= MAX_ITEMS_PER_CHUNK;
        let would_overflow_bytes = !cur.is_empty() && cur_bytes + bytes > MAX_BYTES_PER_CHUNK;
        if would_overflow_items || would_overflow_bytes {
            out.push(std::mem::take(&mut cur));
            cur_bytes = 2;
        }
        cur_bytes += bytes;
        cur.push(v);
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn ddtags(kind: &str, agent: Option<&str>, model: Option<&str>) -> String {
    let mut parts = vec![format!("source:kaizen"), format!("kaizen.type:{kind}")];
    if let Some(a) = agent {
        parts.push(format!("agent:{a}"));
    }
    if let Some(m) = model {
        parts.push(format!("model:{m}"));
    }
    parts.join(",")
}

fn item_timestamp_ms(item: &CanonicalItem) -> i64 {
    let ms = match item {
        CanonicalItem::Event(i) => i.event.ts_ms,
        CanonicalItem::ToolSpan(i) => i.span.started_at_ms.or(i.span.ended_at_ms).unwrap_or(0),
        CanonicalItem::RepoSnapshotChunk(i) => i.chunk.indexed_at_ms,
        CanonicalItem::WorkspaceFactSnapshot { .. } => now_ms(),
    };
    ms.try_into().unwrap_or(i64::MAX)
}

fn item_agent_model(item: &CanonicalItem) -> (Option<String>, Option<String>) {
    match item {
        CanonicalItem::Event(i) => (Some(i.event.agent.clone()), Some(i.event.model.clone())),
        _ => (None, None),
    }
}

fn promote_top_level(m: &mut serde_json::Map<String, Value>, item: &CanonicalItem) {
    let env = match item {
        CanonicalItem::Event(i) => &i.envelope,
        CanonicalItem::ToolSpan(i) => &i.envelope,
        CanonicalItem::RepoSnapshotChunk(i) => &i.envelope,
        CanonicalItem::WorkspaceFactSnapshot { envelope, .. } => envelope,
    };
    m.insert(
        "workspace_hash".into(),
        Value::String(env.workspace_hash.clone()),
    );
    match item {
        CanonicalItem::Event(i) => {
            m.insert(
                "session_id_hash".into(),
                Value::String(i.event.session_id_hash.clone()),
            );
            m.insert("kind".into(), Value::String(i.event.kind.clone()));
            insert_some(m, "tool", i.event.tool.clone().map(Value::String));
            insert_some(m, "tokens_in", i.event.tokens_in.map(|n| json!(n)));
            insert_some(m, "tokens_out", i.event.tokens_out.map(|n| json!(n)));
            insert_some(m, "cost_usd_e6", i.event.cost_usd_e6.map(|n| json!(n)));
        }
        CanonicalItem::ToolSpan(i) => {
            m.insert(
                "session_id_hash".into(),
                Value::String(i.span.session_id_hash.clone()),
            );
            insert_some(m, "tool", i.span.tool.clone().map(Value::String));
            m.insert("status".into(), Value::String(i.span.status.clone()));
        }
        CanonicalItem::RepoSnapshotChunk(i) => {
            m.insert(
                "snapshot_id_hash".into(),
                Value::String(i.chunk.snapshot_id_hash.clone()),
            );
        }
        CanonicalItem::WorkspaceFactSnapshot { .. } => {}
    }
}

fn insert_some(m: &mut serde_json::Map<String, Value>, key: &str, v: Option<Value>) {
    if let Some(v) = v {
        m.insert(key.into(), v);
    }
}

fn short_message(item: &CanonicalItem, agent: Option<&str>, model: Option<&str>) -> String {
    let kind = item.telemetry_kind();
    let agent = agent.unwrap_or("kaizen");
    let model = model.unwrap_or("-");
    match item {
        CanonicalItem::Event(i) => format!(
            "{agent} {model} {} tokens_in={} tokens_out={}",
            i.event.kind,
            i.event.tokens_in.unwrap_or(0),
            i.event.tokens_out.unwrap_or(0),
        ),
        _ => kind.to_string(),
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
