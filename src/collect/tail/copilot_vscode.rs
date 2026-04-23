// SPDX-License-Identifier: AGPL-3.0-or-later
//! Ingest VS Code GitHub Copilot Chat sessions from workspace storage `chatSessions/`.

use crate::core::event::{Event, EventKind, EventSource, SessionRecord, SessionStatus};
use anyhow::Result;
use serde_json::Value;
use std::path::{Path, PathBuf};

const AGENT: &str = "copilot-vscode";

fn canonical(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

fn paths_equal(a: &Path, b: &Path) -> bool {
    canonical(a) == canonical(b)
}

/// VS Code `workspaceStorage` roots for stable + insiders on this OS.
pub fn vscode_workspace_storage_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(home) = std::env::var("HOME") {
        let h = PathBuf::from(home);
        #[cfg(target_os = "macos")]
        {
            let base = h.join("Library/Application Support");
            roots.push(base.join("Code/User/workspaceStorage"));
            roots.push(base.join("Code - Insiders/User/workspaceStorage"));
        }
        #[cfg(target_os = "linux")]
        {
            roots.push(h.join(".config/Code/User/workspaceStorage"));
            roots.push(h.join(".config/Code - Insiders/User/workspaceStorage"));
        }
        #[cfg(target_os = "windows")]
        {
            if let Ok(app) = std::env::var("APPDATA") {
                let base = PathBuf::from(app);
                roots.push(base.join("Code/User/workspaceStorage"));
                roots.push(base.join("Code - Insiders/User/workspaceStorage"));
            }
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        {
            roots.push(h.join(".config/Code/User/workspaceStorage"));
            roots.push(h.join(".config/Code - Insiders/User/workspaceStorage"));
        }
    }
    roots
}

/// Read `workspace.json` in a workspaceStorage hash folder; return folder path if it matches `workspace`.
pub fn vscode_storage_folder_matches(storage_entry: &Path, workspace: &Path) -> bool {
    let wj = storage_entry.join("workspace.json");
    let Ok(text) = std::fs::read_to_string(&wj) else {
        return false;
    };
    let Ok(v) = serde_json::from_str::<Value>(&text) else {
        return false;
    };
    if let Some(folder) = v.get("folder").and_then(|f| f.as_str()) {
        let p = folder.strip_prefix("file://").unwrap_or(folder);
        return paths_equal(&PathBuf::from(p), workspace);
    }
    false
}

fn parse_vscode_copilot_json(path: &Path, session_id: &str) -> Result<Vec<Event>> {
    let text = std::fs::read_to_string(path)?;
    let v: Value = serde_json::from_str(&text)?;
    let mut events = Vec::new();
    let mut seq: u64 = 0;

    if let Some(turns) = v.get("turns").and_then(|t| t.as_array()) {
        for turn in turns {
            let ts_ms = turn
                .get("timestamp")
                .and_then(|t| t.as_str())
                .and_then(parse_iso_or_ms)
                .unwrap_or_else(|| seq.saturating_mul(1000));

            if let Some(reqs) = turn.get("requests").and_then(|r| r.as_array()) {
                for req in reqs {
                    if let Some(tool) = req
                        .get("toolName")
                        .or_else(|| req.get("commandId"))
                        .and_then(|x| x.as_str())
                    {
                        events.push(Event {
                            session_id: session_id.to_string(),
                            seq,
                            ts_ms,
                            ts_exact: false,
                            kind: EventKind::ToolCall,
                            source: EventSource::Tail,
                            tool: Some(tool.to_string()),
                            tool_call_id: req
                                .get("id")
                                .and_then(|x| x.as_str())
                                .map(ToOwned::to_owned),
                            tokens_in: None,
                            tokens_out: None,
                            reasoning_tokens: None,
                            cost_usd_e6: None,
                            payload: req.clone(),
                        });
                        seq += 1;
                    }
                }
            }

            if let Some(parts) = turn.get("response").and_then(|r| r.as_array()) {
                for part in parts {
                    if let Some(kind) = part.get("kind").and_then(|k| k.as_str())
                        && (kind == "toolInvocation" || kind == "toolCall")
                    {
                        let tool = part
                            .get("name")
                            .or_else(|| part.get("toolName"))
                            .and_then(|x| x.as_str())
                            .unwrap_or("tool");
                        events.push(Event {
                            session_id: session_id.to_string(),
                            seq,
                            ts_ms,
                            ts_exact: false,
                            kind: EventKind::ToolCall,
                            source: EventSource::Tail,
                            tool: Some(tool.to_string()),
                            tool_call_id: part
                                .get("id")
                                .and_then(|x| x.as_str())
                                .map(ToOwned::to_owned),
                            tokens_in: None,
                            tokens_out: None,
                            reasoning_tokens: None,
                            cost_usd_e6: None,
                            payload: part.clone(),
                        });
                        seq += 1;
                    }
                }
            }
        }
    }

    Ok(events)
}

fn parse_iso_or_ms(s: &str) -> Option<u64> {
    if let Ok(ms) = s.parse::<u64>() {
        return Some(ms);
    }
    // Minimal ISO-ish fallback: take digits for rough ordering — optional
    None
}

/// Parse Copilot chat `.jsonl` (VS Code internal patch format) — best-effort tool extraction.
fn parse_vscode_copilot_jsonl(path: &Path, session_id: &str) -> Result<Vec<Event>> {
    let text = std::fs::read_to_string(path)?;
    let mut events = Vec::new();
    let mut seq: u64 = 0;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(entry) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let kind = entry.get("kind").and_then(|k| k.as_u64()).unwrap_or(999);
        if kind != 2 {
            continue;
        }
        let Some(val) = entry.get("v") else { continue };
        let Some(reqs) = val.as_array() else { continue };
        for req in reqs {
            if let Some(tool) = req.get("toolName").and_then(|x| x.as_str()) {
                events.push(Event {
                    session_id: session_id.to_string(),
                    seq,
                    ts_ms: seq.saturating_mul(100),
                    ts_exact: false,
                    kind: EventKind::ToolCall,
                    source: EventSource::Tail,
                    tool: Some(tool.to_string()),
                    tool_call_id: req
                        .get("id")
                        .and_then(|x| x.as_str())
                        .map(ToOwned::to_owned),
                    tokens_in: None,
                    tokens_out: None,
                    reasoning_tokens: None,
                    cost_usd_e6: None,
                    payload: req.clone(),
                });
                seq += 1;
            }
        }
    }
    Ok(events)
}

/// Scan `chatSessions` under workspace storage entries that match `workspace`.
pub fn scan_copilot_vscode_workspace(workspace: &Path) -> Result<Vec<(SessionRecord, Vec<Event>)>> {
    let ws_canon = canonical(workspace);
    let mut out = Vec::new();
    for root in vscode_workspace_storage_roots() {
        if !root.is_dir() {
            continue;
        }
        for e in std::fs::read_dir(&root)? {
            let e = e?;
            let p = e.path();
            if !p.is_dir() {
                continue;
            }
            if !vscode_storage_folder_matches(&p, &ws_canon) {
                continue;
            }
            let cs = p.join("chatSessions");
            if !cs.is_dir() {
                continue;
            }
            for f in std::fs::read_dir(&cs)? {
                let f = f?;
                let path = f.path();
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                let session_id = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("vscode-copilot")
                    .to_string();

                let events = if name.ends_with(".jsonl") {
                    parse_vscode_copilot_jsonl(&path, &session_id)?
                } else if name.ends_with(".json") {
                    parse_vscode_copilot_json(&path, &session_id)?
                } else {
                    continue;
                };

                if events.is_empty() {
                    continue;
                }

                let started_at_ms = events.first().map(|e| e.ts_ms).unwrap_or(0);
                out.push((
                    SessionRecord {
                        id: format!("vscode-{session_id}"),
                        agent: AGENT.to_string(),
                        model: None,
                        workspace: workspace.to_string_lossy().to_string(),
                        started_at_ms,
                        ended_at_ms: None,
                        status: SessionStatus::Done,
                        trace_path: path.to_string_lossy().to_string(),
                        start_commit: None,
                        end_commit: None,
                        branch: None,
                        dirty_start: None,
                        dirty_end: None,
                        repo_binding_source: None,
                    },
                    events,
                ));
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn workspace_json_matches() {
        let dir = TempDir::new().unwrap();
        let ws = dir.path().join("proj");
        std::fs::create_dir_all(&ws).unwrap();
        let ws_canon = std::fs::canonicalize(&ws).unwrap();

        let hash = dir.path().join("ws-hash");
        std::fs::create_dir_all(&hash).unwrap();
        std::fs::write(
            hash.join("workspace.json"),
            format!(
                r#"{{"folder": "file://{}"}}"#,
                ws_canon.to_string_lossy().replace('\\', "/")
            ),
        )
        .unwrap();

        assert!(vscode_storage_folder_matches(&hash, &ws_canon));
    }

    #[test]
    fn parse_turns_json() {
        let dir = TempDir::new().unwrap();
        let p = dir.path().join("sess.json");
        std::fs::write(
            &p,
            r#"{"turns":[{"timestamp":"1000","requests":[{"toolName":"github.copilot.git"}]}]}"#,
        )
        .unwrap();
        let evs = parse_vscode_copilot_json(&p, "sid").unwrap();
        assert_eq!(evs.len(), 1);
        assert_eq!(evs[0].tool.as_deref(), Some("github.copilot.git"));
    }
}
