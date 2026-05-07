// SPDX-License-Identifier: AGPL-3.0-or-later
//! Pure JSON builder + chunker for Datadog Logs API; HTTP lives in [`super::transport`].

use crate::sync::IngestExportBatch;
use crate::sync::canonical::{CanonicalItem, expand_ingest_batch};
use serde_json::{Value, json};

/// DD Logs API caps, with byte slack for HTTP envelope.
pub const MAX_ITEMS_PER_CHUNK: usize = 1000;
pub const MAX_BYTES_PER_CHUNK: usize = 5 * 1024 * 1024 - 64 * 1024;

pub fn current_hostname() -> String {
    gethostname::gethostname().to_string_lossy().into_owned()
}

pub fn build_log_objects(batch: &IngestExportBatch, hostname: &str) -> Vec<Value> {
    expand_ingest_batch(batch)
        .iter()
        .map(|c| dd_log_object(c, hostname))
        .collect()
}

/// One Datadog log object for a single [`CanonicalItem`].
pub fn dd_log_object(item: &CanonicalItem, hostname: &str) -> Value {
    let kind_tag = item.telemetry_kind();
    let ts_ms = item_timestamp_ms(item);
    let (agent, model) = item_agent_model(item);
    let project_name = item_project_name(item);
    let mut obj = json!({
        "timestamp": ts_ms,
        "hostname": hostname,
        "ddsource": "kaizen",
        "service": "kaizen",
        "ddtags": ddtags(kind_tag, agent.as_deref(), model.as_deref(), project_name),
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

fn ddtags(kind: &str, agent: Option<&str>, model: Option<&str>, project: Option<&str>) -> String {
    let mut parts = vec![format!("source:kaizen"), format!("kaizen.type:{kind}")];
    if let Some(a) = agent {
        parts.push(format!("agent:{a}"));
    }
    if let Some(m) = model {
        parts.push(format!("model:{m}"));
    }
    if let Some(p) = project {
        parts.push(format!("project_name:{p}"));
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

fn item_project_name(item: &CanonicalItem) -> Option<&str> {
    item_envelope(item)
        .identity
        .as_ref()
        .and_then(|i| i.workspace_label.as_deref())
}

fn promote_top_level(m: &mut serde_json::Map<String, Value>, item: &CanonicalItem) {
    let env = item_envelope(item);
    m.insert(
        "workspace_hash".into(),
        Value::String(env.workspace_hash.clone()),
    );
    insert_some(
        m,
        "project_name",
        item_project_name(item).map(|s| Value::String(s.to_string())),
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
            insert_some(
                m,
                "reasoning_tokens",
                i.event.reasoning_tokens.map(|n| json!(n)),
            );
            insert_some(m, "cost_usd_e6", i.event.cost_usd_e6.map(|n| json!(n)));
        }
        CanonicalItem::ToolSpan(i) => {
            m.insert(
                "session_id_hash".into(),
                Value::String(i.span.session_id_hash.clone()),
            );
            insert_some(m, "tool", i.span.tool.clone().map(Value::String));
            m.insert("status".into(), Value::String(i.span.status.clone()));
            insert_some(m, "lead_time_ms", i.span.lead_time_ms.map(|n| json!(n)));
            insert_some(m, "tokens_in", i.span.tokens_in.map(|n| json!(n)));
            insert_some(m, "tokens_out", i.span.tokens_out.map(|n| json!(n)));
            insert_some(
                m,
                "reasoning_tokens",
                i.span.reasoning_tokens.map(|n| json!(n)),
            );
            insert_some(m, "cost_usd_e6", i.span.cost_usd_e6.map(|n| json!(n)));
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

fn item_envelope(item: &CanonicalItem) -> &crate::sync::canonical::CanonicalEnvelope {
    match item {
        CanonicalItem::Event(i) => &i.envelope,
        CanonicalItem::ToolSpan(i) => &i.envelope,
        CanonicalItem::RepoSnapshotChunk(i) => &i.envelope,
        CanonicalItem::WorkspaceFactSnapshot { envelope, .. } => envelope,
    }
}

fn insert_some(m: &mut serde_json::Map<String, Value>, key: &str, v: Option<Value>) {
    if let Some(v) = v {
        m.insert(key.into(), v);
    }
}

fn short_message(item: &CanonicalItem, agent: Option<&str>, model: Option<&str>) -> String {
    let agent = agent.unwrap_or("kaizen");
    let model = model.unwrap_or("-");
    match item {
        CanonicalItem::Event(i) => format!(
            "{agent} {model} {} tokens_in={} tokens_out={}",
            i.event.kind,
            i.event.tokens_in.unwrap_or(0),
            i.event.tokens_out.unwrap_or(0),
        ),
        _ => item.telemetry_kind().to_string(),
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
