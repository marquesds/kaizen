// SPDX-License-Identifier: AGPL-3.0-or-later
//! Ingest Goose sessions from SQLite (`sessions.db`) or legacy `.jsonl` in the same folder.

use crate::collect::model_from_json;
use crate::core::event::{Event, EventKind, EventSource, SessionRecord, SessionStatus};
use anyhow::{Context, Result};
use rusqlite::Connection;
use serde_json::Value;
use std::path::{Path, PathBuf};

const AGENT: &str = "goose";

/// Candidate paths for `sessions.db` (Goose uses platform-specific data dirs).
fn goose_session_db_paths(home: &Path) -> Vec<PathBuf> {
    let mut v = vec![
        home.join("Library/Application Support/Block/goose/data/sessions/sessions.db"),
        home.join(".local/share/goose/sessions/sessions.db"),
    ];
    if let Ok(root) = std::env::var("GOOSE_PATH_ROOT") {
        let b = PathBuf::from(root);
        v.insert(0, b.join("data/sessions/sessions.db"));
    }
    v
}

fn canonical(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

fn paths_equal(a: &Path, b: &Path) -> bool {
    canonical(a) == canonical(b)
}

/// True if DB has Goose-style `sessions` and `messages` tables.
fn is_goose_schema(conn: &Connection) -> Result<bool> {
    let n: i32 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('sessions','messages')",
        [],
        |row| row.get(0),
    )?;
    Ok(n >= 2)
}

fn goose_model_from_row(
    model_config_json: Option<String>,
    provider: Option<String>,
) -> Option<String> {
    if let Some(ref s) = model_config_json
        && let Ok(v) = serde_json::from_str::<Value>(s)
        && let Some(m) = v
            .get("model")
            .or_else(|| v.get("id"))
            .and_then(|x| x.as_str())
    {
        return Some(m.to_string());
    }
    provider
}

/// Parse `content_json` (array of MessageContent blocks) into events for one DB row.
fn events_from_goose_content(
    session_id: &str,
    seq: u64,
    ts_ms: u64,
    tokens: Option<i64>,
    content_json: &str,
) -> Result<Vec<Event>> {
    let v: Value = serde_json::from_str(content_json.trim()).context("goose content_json")?;
    let blocks: Vec<Value> = if let Some(arr) = v.as_array() {
        arr.clone()
    } else if let Some(arr) = v.get("content").and_then(|c| c.as_array()) {
        arr.clone()
    } else {
        return Ok(vec![]);
    };

    let mut out = Vec::new();
    let mut s = seq;
    let tk = tokens.map(|t| t.max(0) as u32);
    for block in blocks {
        let Some(typ) = block.get("type").and_then(|t| t.as_str()) else {
            continue;
        };
        match typ {
            "toolRequest" | "frontendToolRequest" => {
                let id = block
                    .get("id")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                let tool_name = block
                    .get("toolCall")
                    .and_then(|tc| tc.get("Ok"))
                    .and_then(|ok| ok.get("name"))
                    .and_then(|n| n.as_str())
                    .or_else(|| {
                        block
                            .get("toolCall")
                            .and_then(|tc| tc.get("name"))
                            .and_then(|n| n.as_str())
                    })
                    .unwrap_or("")
                    .to_string();
                out.push(Event {
                    session_id: session_id.to_string(),
                    seq: s,
                    ts_ms,
                    ts_exact: true,
                    kind: EventKind::ToolCall,
                    source: EventSource::Tail,
                    tool: Some(tool_name),
                    tool_call_id: Some(id),
                    tokens_in: None,
                    tokens_out: tk,
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
                    payload: block,
                });
                s += 1;
            }
            "toolResponse" => {
                let id = block
                    .get("id")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                out.push(Event {
                    session_id: session_id.to_string(),
                    seq: s,
                    ts_ms,
                    ts_exact: true,
                    kind: EventKind::ToolResult,
                    source: EventSource::Tail,
                    tool: None,
                    tool_call_id: Some(id),
                    tokens_in: None,
                    tokens_out: tk,
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
                    payload: block,
                });
                s += 1;
            }
            _ => {}
        }
    }
    Ok(out)
}

fn sessions_select_sql(conn: &Connection) -> &'static str {
    let Ok(mut stmt) = conn.prepare("PRAGMA table_info(sessions)") else {
        return "SELECT id, working_dir FROM sessions";
    };
    let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(1)) else {
        return "SELECT id, working_dir FROM sessions";
    };
    let mut cols = Vec::new();
    for c in rows.flatten() {
        cols.push(c);
    }
    let has = |n: &str| cols.iter().any(|c| c == n);
    if has("model_config_json")
        && has("provider_name")
        && has("input_tokens")
        && has("output_tokens")
    {
        "SELECT id, working_dir, model_config_json, provider_name, input_tokens, output_tokens FROM sessions"
    } else {
        "SELECT id, working_dir FROM sessions"
    }
}

/// Read all sessions for `workspace` from a Goose `sessions.db`.
pub fn scan_goose_sqlite(
    db_path: &Path,
    workspace: &Path,
) -> Result<Vec<(SessionRecord, Vec<Event>)>> {
    let ws_canon = canonical(workspace);
    let conn = Connection::open(db_path).with_context(|| format!("open {}", db_path.display()))?;
    if !is_goose_schema(&conn)? {
        return Ok(vec![]);
    }

    let sql = sessions_select_sql(&conn);
    let mut stmt = conn
        .prepare(sql)
        .with_context(|| format!("goose sessions query ({sql})"))?;

    let mut sessions_out = Vec::new();
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let id: String = row.get(0)?;
        let working_dir: String = row.get(1)?;
        let (model_json, provider, in_tok, out_tok) = if sql.contains("model_config_json") {
            (
                row.get::<_, Option<String>>(2).ok().flatten(),
                row.get::<_, Option<String>>(3).ok().flatten(),
                row.get::<_, Option<i64>>(4).ok().flatten(),
                row.get::<_, Option<i64>>(5).ok().flatten(),
            )
        } else {
            (None, None, None, None)
        };

        let wd = PathBuf::from(&working_dir);
        if !paths_equal(&wd, &ws_canon) {
            continue;
        }

        let model = goose_model_from_row(model_json, provider);

        let mut msg_stmt = conn.prepare(
            "SELECT content_json, created_timestamp, tokens FROM messages WHERE session_id = ?1 ORDER BY id",
        )?;
        let msg_rows = msg_stmt.query_map([&id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, Option<i64>>(2)?,
            ))
        })?;

        let mut events = Vec::new();
        let mut seq: u64 = 0;
        for mr in msg_rows {
            let (content_json, created_ts, tokens) = mr?;
            let ts_ms = (created_ts.max(0) as u64).saturating_mul(1000);
            let chunk = events_from_goose_content(&id, seq, ts_ms, tokens, &content_json)?;
            let n = chunk.len() as u64;
            events.extend(chunk);
            seq += n.max(1);
        }

        let started_at_ms = events.first().map(|e| e.ts_ms).unwrap_or_else(|| {
            std::fs::metadata(db_path)
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0)
        });

        let record = SessionRecord {
            id: id.clone(),
            agent: AGENT.to_string(),
            model,
            workspace: workspace.to_string_lossy().to_string(),
            started_at_ms,
            ended_at_ms: None,
            status: SessionStatus::Done,
            trace_path: db_path.to_string_lossy().to_string(),
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
        };

        if let (Some(ti), Some(to)) = (in_tok, out_tok)
            && (ti > 0 || to > 0)
            && let Some(last) = events.last_mut()
        {
            last.tokens_in = Some(ti.max(0) as u32);
            last.tokens_out = Some(to.max(0) as u32);
        }

        sessions_out.push((record, events));
    }

    Ok(sessions_out)
}

/// Legacy flat `.jsonl` session files (pre–SQLite migration).
pub fn scan_goose_legacy_jsonl_dir(
    sessions_dir: &Path,
    workspace: &Path,
) -> Result<Vec<(SessionRecord, Vec<Event>)>> {
    let ws_canon = canonical(workspace);
    let mut out = Vec::new();
    if !sessions_dir.is_dir() {
        return Ok(out);
    }
    for entry in std::fs::read_dir(sessions_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map(|e| e != "jsonl").unwrap_or(true) {
            continue;
        }
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(_) => continue,
        };
        let session_id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("goose-legacy")
            .to_string();

        let mut events = Vec::new();
        let mut seq: u64 = 0;
        let mut model: Option<String> = None;
        let mut matches_ws = false;

        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let Ok(v) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            if let Some(wd) = v
                .get("working_dir")
                .or_else(|| v.get("workingDir"))
                .and_then(|x| x.as_str())
                && paths_equal(Path::new(wd), &ws_canon)
            {
                matches_ws = true;
            }
            if model.is_none() {
                model = model_from_json::from_value(&v);
            }
            if let Some(created) = v.get("created").and_then(|c| c.as_i64()) {
                let ts_ms = (created.max(0) as u64).saturating_mul(1000);
                if let Some(content) = v.get("content") {
                    let s = content.to_string();
                    if let Ok(chunk) = events_from_goose_content(&session_id, seq, ts_ms, None, &s)
                    {
                        let n = chunk.len() as u64;
                        events.extend(chunk);
                        seq += n.max(1);
                    }
                }
            }
        }

        if !matches_ws && !events.is_empty() {
            // Heuristic: if no working_dir in file, skip (avoid cross-workspace noise)
            continue;
        }
        if events.is_empty() {
            continue;
        }

        out.push((
            SessionRecord {
                id: session_id,
                agent: AGENT.to_string(),
                model,
                workspace: workspace.to_string_lossy().to_string(),
                started_at_ms: events.first().map(|e| e.ts_ms).unwrap_or(0),
                ended_at_ms: None,
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
            },
            events,
        ));
    }
    Ok(out)
}

/// All Goose sessions for a workspace (SQLite first, then legacy jsonl in parent dir).
pub fn scan_goose_workspace(
    home: &Path,
    workspace: &Path,
) -> Result<Vec<(SessionRecord, Vec<Event>)>> {
    let mut all = Vec::new();
    for db in goose_session_db_paths(home) {
        if db.is_file() {
            all.extend(scan_goose_sqlite(&db, workspace)?);
        }
    }
    for db in goose_session_db_paths(home) {
        let parent = db.parent().unwrap_or_else(|| Path::new("."));
        if parent.is_dir() {
            all.extend(scan_goose_legacy_jsonl_dir(parent, workspace)?);
        }
    }
    Ok(all)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn goose_sqlite_roundtrip_fixture() {
        let dir = TempDir::new().unwrap();
        let ws = dir.path().join("proj");
        std::fs::create_dir_all(&ws).unwrap();
        let ws_canon = std::fs::canonicalize(&ws).unwrap();

        let db_path = dir.path().join("sessions.db");
        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch(
            r"
            CREATE TABLE schema_version (version INTEGER PRIMARY KEY);
            INSERT INTO schema_version (version) VALUES (7);
            CREATE TABLE sessions (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL DEFAULT '',
                description TEXT NOT NULL DEFAULT '',
                working_dir TEXT NOT NULL,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                model_config_json TEXT,
                provider_name TEXT,
                input_tokens INTEGER,
                output_tokens INTEGER,
                total_tokens INTEGER
            );
            CREATE TABLE messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                message_id TEXT,
                session_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content_json TEXT NOT NULL,
                created_timestamp INTEGER NOT NULL,
                tokens INTEGER,
                metadata_json TEXT
            );
            ",
        )
        .unwrap();

        conn.execute(
            "INSERT INTO sessions (id, working_dir, model_config_json, provider_name) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![
                "sess-goose-1",
                ws_canon.to_string_lossy().as_ref(),
                r#"{"model":"gpt-4o-fixture"}"#,
                "openai"
            ],
        )
        .unwrap();

        let tool_req = r#"[{"type":"toolRequest","id":"t1","toolCall":{"Ok":{"name":"shell","arguments":{}}}}]"#;
        conn.execute(
            "INSERT INTO messages (message_id, session_id, role, content_json, created_timestamp) VALUES ('m1', 'sess-goose-1', 'assistant', ?1, 1700000000)",
            [tool_req],
        )
        .unwrap();

        let sessions = scan_goose_sqlite(&db_path, &ws_canon).unwrap();
        assert_eq!(sessions.len(), 1);
        let (rec, evs) = &sessions[0];
        assert_eq!(rec.agent, "goose");
        assert_eq!(rec.model.as_deref(), Some("gpt-4o-fixture"));
        assert!(!evs.is_empty());
        assert_eq!(evs[0].kind, EventKind::ToolCall);
        assert_eq!(evs[0].tool.as_deref(), Some("shell"));
    }

    #[test]
    fn goose_skips_other_workspace() {
        let dir = TempDir::new().unwrap();
        let ws_a = dir.path().join("a");
        let ws_b = dir.path().join("b");
        std::fs::create_dir_all(&ws_a).unwrap();
        std::fs::create_dir_all(&ws_b).unwrap();
        let canon_b = std::fs::canonicalize(&ws_b).unwrap();

        let db_path = dir.path().join("sessions.db");
        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch(
            r"
            CREATE TABLE sessions (id TEXT PRIMARY KEY, working_dir TEXT NOT NULL);
            CREATE TABLE messages (id INTEGER PRIMARY KEY AUTOINCREMENT, session_id TEXT, role TEXT, content_json TEXT NOT NULL, created_timestamp INTEGER NOT NULL);
            ",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO sessions (id, working_dir) VALUES ('x', ?1)",
            [canon_b.to_string_lossy().as_ref()],
        )
        .unwrap();

        let sessions = scan_goose_sqlite(&db_path, &ws_a).unwrap();
        assert!(sessions.is_empty());
    }
}
