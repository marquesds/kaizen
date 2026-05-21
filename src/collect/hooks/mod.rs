// SPDX-License-Identifier: AGPL-3.0-or-later
//! Hook event types shared by Cursor and Claude Code parsers.

pub mod claude;
pub mod cursor;
pub mod normalize;
pub mod openclaw;
pub mod vibe;

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
    PermissionRequest,
    UserPromptSubmit,
    Notification,
    SubagentStart,
    SubagentStop,
    Interrupt,
    ModeTransition,
    Unknown(String),
}

impl EventKind {
    pub fn parse(s: &str) -> Self {
        match s {
            "PreToolUse" | "pre_tool_use" => Self::PreToolUse,
            "PostToolUse" | "post_tool_use" => Self::PostToolUse,
            "Stop" | "stop" => Self::Stop,
            "SessionStart" | "session_start" => Self::SessionStart,
            "PermissionRequest" | "permission_request" => Self::PermissionRequest,
            "UserPromptSubmit" | "user_prompt_submit" => Self::UserPromptSubmit,
            "Notification" | "notification" => Self::Notification,
            "SubagentStart" | "subagent_start" => Self::SubagentStart,
            "SubagentStop" | "subagent_stop" => Self::SubagentStop,
            "Interrupt" | "interrupt" => Self::Interrupt,
            "ModeTransition" | "mode_transition" => Self::ModeTransition,
            other => Self::Unknown(other.to_string()),
        }
    }
}
