// SPDX-License-Identifier: AGPL-3.0-or-later
//! Codex Desktop date-sharded session logs under `~/.codex/sessions`.

use crate::collect::model_from_json;
use crate::collect::tail::codex_desktop_event::parse_modern_line;
use crate::core::event::{Event, SessionRecord, SessionStatus};
use anyhow::{Context, Result};
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Default)]
struct Meta {
    id: Option<String>,
    workspace: Option<String>,
    model: Option<String>,
    started_ms: Option<u64>,
    ended_ms: Option<u64>,
    agent_version: Option<String>,
}

pub fn scan_codex_sessions_root(
    root: &Path,
    workspace: &Path,
) -> Result<Vec<(SessionRecord, Vec<Event>)>> {
    if !root.exists() {
        return Ok(Vec::new());
    }
    let target = crate::core::paths::canonical(workspace);
    let mut paths = Vec::new();
    collect_jsonl(root, &mut paths)?;
    paths.sort();
    Ok(paths
        .into_iter()
        .filter_map(|p| scan_codex_session_file(&p).ok())
        .filter(|(r, _)| workspace_matches(&r.workspace, &target))
        .collect())
}

pub fn scan_codex_session_file(path: &Path) -> Result<(SessionRecord, Vec<Event>)> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("read codex file: {}", path.display()))?;
    let meta = content.lines().fold(Meta::default(), read_meta);
    let id = meta.id.clone().unwrap_or_else(|| file_stem(path));
    let base = meta.started_ms.unwrap_or_else(|| file_mtime_ms(path));
    let events = content
        .lines()
        .enumerate()
        .filter_map(|(i, line)| parse_modern_line(&id, i as u64, base, meta.model.as_deref(), line))
        .collect();
    Ok((record(path, id, meta, base), events))
}

fn collect_jsonl(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        match (path.is_dir(), path.extension().and_then(|x| x.to_str())) {
            (true, _) => collect_jsonl(&path, out)?,
            (false, Some("jsonl")) => out.push(path),
            _ => {}
        }
    }
    Ok(())
}

fn read_meta(mut meta: Meta, line: &str) -> Meta {
    let Ok(v) = serde_json::from_str::<Value>(line.trim()) else {
        return meta;
    };
    let Some(obj) = v.as_object() else {
        return meta;
    };
    let payload = obj.get("payload").unwrap_or(&Value::Null);
    if let Some(ts) = line_ts(obj, payload) {
        meta.started_ms = Some(meta.started_ms.map_or(ts, |v| v.min(ts)));
        meta.ended_ms = Some(meta.ended_ms.map_or(ts, |v| v.max(ts)));
    }
    read_meta_payload(&mut meta, obj, payload);
    meta
}

fn read_meta_payload(meta: &mut Meta, obj: &serde_json::Map<String, Value>, payload: &Value) {
    match obj.get("type").and_then(Value::as_str).unwrap_or("") {
        "session_meta" => {
            meta.id = text(payload, "id").or(meta.id.take());
            meta.workspace = text(payload, "cwd").or(meta.workspace.take());
            meta.agent_version = text(payload, "cli_version").or(meta.agent_version.take());
        }
        "turn_context" => {
            meta.workspace = text(payload, "cwd").or(meta.workspace.take());
            meta.model = text(payload, "model").or(meta.model.take());
        }
        _ => meta.model = model_from_json::from_value(payload).or(meta.model.take()),
    }
}

fn record(path: &Path, id: String, meta: Meta, base: u64) -> SessionRecord {
    SessionRecord {
        id,
        agent: "codex".into(),
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
        agent_version: meta.agent_version,
        os: None,
        arch: None,
        repo_file_count: None,
        repo_total_loc: None,
    }
}

fn line_ts(obj: &serde_json::Map<String, Value>, payload: &Value) -> Option<u64> {
    ["timestamp_ms", "ts_ms", "created_at_ms", "timestamp"]
        .iter()
        .find_map(|k| obj.get(*k).and_then(crate::collect::tail::value_ts_ms))
        .or_else(|| {
            payload
                .get("timestamp")
                .and_then(crate::collect::tail::value_ts_ms)
        })
}

fn workspace_matches(found: &str, target: &Path) -> bool {
    !found.is_empty() && crate::core::paths::canonical(Path::new(found)) == target
}

fn text(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(ToOwned::to_owned)
}

fn file_stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string()
}

fn file_mtime_ms(path: &Path) -> u64 {
    path.metadata()
        .ok()
        .and_then(|m| m.modified().ok())
        .map(|t| {
            t.duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64
        })
        .unwrap_or(0)
}
