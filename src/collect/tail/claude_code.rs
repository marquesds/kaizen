// SPDX-License-Identifier: AGPL-3.0-or-later
//! Claude Code project logs stored as top-level project `*.jsonl` files.

use crate::collect::model_from_json;
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

pub fn scan_claude_project_dir(
    project_dir: &Path,
    workspace: &Path,
) -> Result<Vec<(SessionRecord, Vec<Event>)>> {
    if !project_dir.exists() {
        return Ok(Vec::new());
    }
    let target = crate::core::paths::canonical(workspace);
    let mut out = Vec::new();
    for file in top_level_jsonl(project_dir)? {
        push_if_target(
            &mut out,
            scan_claude_session_file(&file, None, Some(workspace))?,
            &target,
        );
    }
    for (file, parent) in subagent_jsonl(project_dir)? {
        push_if_target(
            &mut out,
            scan_claude_session_file(&file, Some(parent), Some(workspace))?,
            &target,
        );
    }
    Ok(out)
}

pub fn scan_claude_session_file(
    path: &Path,
    parent_session_id: Option<String>,
    workspace_fallback: Option<&Path>,
) -> Result<(SessionRecord, Vec<Event>)> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("read claude file: {}", path.display()))?;
    let mut meta = content.lines().fold(Meta::default(), read_meta);
    if meta.workspace.is_none() {
        meta.workspace = workspace_fallback.map(|p| p.to_string_lossy().to_string());
    }
    let id = meta.id.clone().unwrap_or_else(|| file_stem(path));
    let base = meta.started_ms.unwrap_or_else(|| file_mtime_ms(path));
    let events = content
        .lines()
        .enumerate()
        .filter_map(|(i, line)| {
            crate::collect::tail::claude::parse_claude_line(&id, i as u64, base, line)
                .ok()
                .flatten()
        })
        .collect();
    Ok((record(path, id, meta, base, parent_session_id), events))
}

fn top_level_jsonl(project_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut out = std::fs::read_dir(project_dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_file() && p.extension().and_then(|x| x.to_str()) == Some("jsonl"))
        .collect::<Vec<_>>();
    out.sort();
    Ok(out)
}

fn subagent_jsonl(project_dir: &Path) -> Result<Vec<(PathBuf, String)>> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(project_dir)? {
        let path = entry?.path();
        let Some(parent) = path.file_name().and_then(|n| n.to_str()).map(str::to_owned) else {
            continue;
        };
        collect_subagents(&path.join("subagents"), &parent, &mut out)?;
    }
    out.sort();
    Ok(out)
}

fn collect_subagents(dir: &Path, parent: &str, out: &mut Vec<(PathBuf, String)>) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.extension().and_then(|x| x.to_str()) == Some("jsonl") {
            out.push((path, parent.to_string()));
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
    if let Some(ts) = line_ts(obj) {
        meta.started_ms = Some(meta.started_ms.map_or(ts, |v| v.min(ts)));
        meta.ended_ms = Some(meta.ended_ms.map_or(ts, |v| v.max(ts)));
    }
    meta.id = text(&v, "sessionId").or(meta.id);
    meta.workspace = text(&v, "cwd").or(meta.workspace);
    meta.agent_version = text(&v, "version").or(meta.agent_version);
    meta.model = model_from_json::from_value(&v).or(meta.model);
    meta
}

fn record(
    path: &Path,
    id: String,
    meta: Meta,
    base: u64,
    parent_session_id: Option<String>,
) -> SessionRecord {
    SessionRecord {
        id,
        agent: "claude".into(),
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
        parent_session_id,
        agent_version: meta.agent_version,
        os: None,
        arch: None,
        repo_file_count: None,
        repo_total_loc: None,
    }
}

fn push_if_target(
    out: &mut Vec<(SessionRecord, Vec<Event>)>,
    row: (SessionRecord, Vec<Event>),
    target: &Path,
) {
    if workspace_matches(&row.0.workspace, target) {
        out.push(row);
    }
}

fn workspace_matches(found: &str, target: &Path) -> bool {
    !found.is_empty() && crate::core::paths::canonical(Path::new(found)) == target
}

fn line_ts(obj: &serde_json::Map<String, Value>) -> Option<u64> {
    ["timestamp_ms", "ts_ms", "created_at_ms", "timestamp"]
        .iter()
        .find_map(|k| obj.get(*k).and_then(crate::collect::tail::value_ts_ms))
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
