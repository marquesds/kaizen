// SPDX-License-Identifier: AGPL-3.0-or-later

use super::modern_jsonl_event::parse_common_line;
use super::modern_jsonl_fields::{
    file_mtime_ms, file_stem, json, session_id, timestamp, workspace,
};
use crate::collect::model_from_json;
use crate::core::event::{Event, SessionRecord, SessionStatus};
use anyhow::{Context, Result};
use serde_json::Value;
use std::path::Path;

#[derive(Default)]
struct Meta {
    id: Option<String>,
    workspace: Option<String>,
    model: Option<String>,
    started_ms: Option<u64>,
    ended_ms: Option<u64>,
}

pub(crate) fn scan_agent_session_file(
    path: &Path,
    agent: &str,
) -> Result<(SessionRecord, Vec<Event>)> {
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let meta = text
        .lines()
        .filter_map(json)
        .fold(Meta::default(), read_meta);
    let id = meta.id.clone().unwrap_or_else(|| file_stem(path));
    let base = meta.started_ms.unwrap_or_else(|| file_mtime_ms(path));
    let events = text
        .lines()
        .enumerate()
        .filter_map(|(i, line)| parse_common_line(agent, &id, i as u64, base, line))
        .collect();
    Ok((record(path, agent, id, meta, base), events))
}

fn read_meta(mut meta: Meta, v: Value) -> Meta {
    if let Some(ts) = timestamp(&v) {
        meta.started_ms = Some(meta.started_ms.map_or(ts, |old| old.min(ts)));
        meta.ended_ms = Some(meta.ended_ms.map_or(ts, |old| old.max(ts)));
    }
    meta.id = session_id(&v).or(meta.id);
    meta.workspace = workspace(&v).or(meta.workspace);
    meta.model = model_from_json::from_value(&v).or(meta.model);
    meta
}

fn record(path: &Path, agent: &str, id: String, meta: Meta, base: u64) -> SessionRecord {
    SessionRecord {
        id,
        agent: agent.to_string(),
        model: meta.model,
        workspace: meta.workspace.unwrap_or_default(),
        started_at_ms: base,
        ended_at_ms: meta.ended_ms,
        status: SessionStatus::Done,
        trace_path: path.to_string_lossy().to_string(),
        start_commit: None,
        end_commit: None,
        branch: None,
        dirty_start: None,
        dirty_end: None,
        repo_binding_source: None,
        prompt_fingerprint: None,
        parent_session_id: None,
        agent_version: None,
        os: None,
        arch: None,
        repo_file_count: None,
        repo_total_loc: None,
    }
}
