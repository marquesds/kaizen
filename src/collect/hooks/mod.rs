//! Hook event types shared by Cursor and Claude Code parsers.

pub mod claude;
pub mod cursor;
pub mod normalize;

/// Normalized event emitted by any hook parser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookEvent {
    pub kind: EventKind,
    pub session_id: String,
    pub ts_ms: u64,
    pub payload: serde_json::Value,
}

/// Hook event kinds recognized across agents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventKind {
    PreToolUse,
    PostToolUse,
    Stop,
    SessionStart,
    Unknown(String),
}

impl EventKind {
    pub fn parse(s: &str) -> Self {
        match s {
            "PreToolUse" | "pre_tool_use" => Self::PreToolUse,
            "PostToolUse" | "post_tool_use" => Self::PostToolUse,
            "Stop" | "stop" => Self::Stop,
            "SessionStart" | "session_start" => Self::SessionStart,
            other => Self::Unknown(other.to_string()),
        }
    }
}
