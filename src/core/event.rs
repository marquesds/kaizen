// SPDX-License-Identifier: AGPL-3.0-or-later
//! Core event + session-record types. Pure data, no IO.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventKind {
    ToolCall,
    ToolResult,
    Message,
    Error,
    Cost,
    Hook,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventSource {
    Tail,
    Hook,
    Proxy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub session_id: String,
    pub seq: u64,
    pub ts_ms: u64,
    pub ts_exact: bool,
    pub kind: EventKind,
    pub source: EventSource,
    pub tool: Option<String>,
    pub tool_call_id: Option<String>,
    pub tokens_in: Option<u32>,
    pub tokens_out: Option<u32>,
    pub reasoning_tokens: Option<u32>,
    pub cost_usd_e6: Option<i64>,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionStatus {
    Running,
    Waiting,
    Idle,
    Done,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    pub id: String,
    pub agent: String,
    pub model: Option<String>,
    pub workspace: String,
    pub started_at_ms: u64,
    pub ended_at_ms: Option<u64>,
    pub status: SessionStatus,
    pub trace_path: String,
    pub start_commit: Option<String>,
    pub end_commit: Option<String>,
    pub branch: Option<String>,
    pub dirty_start: Option<bool>,
    pub dirty_end: Option<bool>,
    pub repo_binding_source: Option<String>,
    pub prompt_fingerprint: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn event_serde_round_trip() {
        let e = Event {
            session_id: "s1".to_string(),
            seq: 0,
            ts_ms: 1000,
            ts_exact: false,
            kind: EventKind::ToolCall,
            source: EventSource::Tail,
            tool: Some("read_file".to_string()),
            tool_call_id: Some("call_1".to_string()),
            tokens_in: None,
            tokens_out: None,
            reasoning_tokens: None,
            cost_usd_e6: None,
            payload: json!({"path": "src/main.rs"}),
        };
        let s = serde_json::to_string(&e).unwrap();
        let e2: Event = serde_json::from_str(&s).unwrap();
        assert_eq!(e.session_id, e2.session_id);
        assert_eq!(e.kind, e2.kind);
        assert_eq!(e.tool, e2.tool);
    }

    #[test]
    fn session_record_serde_round_trip() {
        let r = SessionRecord {
            id: "abc".to_string(),
            agent: "cursor".to_string(),
            model: Some("gpt-4".to_string()),
            workspace: "/home/user/proj".to_string(),
            started_at_ms: 0,
            ended_at_ms: Some(9999),
            status: SessionStatus::Done,
            trace_path: "/tmp/abc".to_string(),
            start_commit: Some("abc".to_string()),
            end_commit: Some("def".to_string()),
            branch: Some("main".to_string()),
            dirty_start: Some(false),
            dirty_end: Some(true),
            repo_binding_source: Some("git".to_string()),
            prompt_fingerprint: None,
        };
        let s = serde_json::to_string(&r).unwrap();
        let r2: SessionRecord = serde_json::from_str(&s).unwrap();
        assert_eq!(r.id, r2.id);
        assert_eq!(r.status, r2.status);
        assert_eq!(r.ended_at_ms, r2.ended_at_ms);
    }
}
