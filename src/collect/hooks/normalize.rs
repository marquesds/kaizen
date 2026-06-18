// SPDX-License-Identifier: AGPL-3.0-or-later
//! Map HookEvent → core Event + derive SessionStatus.

use crate::collect::hooks::{EventKind as HookKind, HookEvent};
use crate::core::event::{Event, EventKind, EventSource, SessionStatus};

/// Map HookEvent → Event. seq caller-supplied.
pub fn hook_to_event(h: &HookEvent, seq: u64) -> Event {
    let cost_usd_e6 = h
        .payload
        .get("total_cost_usd")
        .and_then(|v| v.as_f64())
        .map(|f| (f * 1_000_000.0) as i64);
    let lifecycle = lifecycle_type(&h.kind);
    Event {
        session_id: h.session_id.clone(),
        seq,
        ts_ms: h.ts_ms,
        ts_exact: true,
        kind: core_event_kind(&h.kind),
        source: EventSource::Hook,
        tool: hook_tool(&h.payload),
        tool_call_id: hook_tool_id(&h.payload),
        tokens_in: u32_field(&h.payload, "input_tokens"),
        tokens_out: u32_field(&h.payload, "output_tokens"),
        reasoning_tokens: u32_field(&h.payload, "reasoning_tokens"),
        cost_usd_e6,
        stop_reason: text_field(&h.payload, "stop_reason"),
        latency_ms: u32_field(&h.payload, "latency_ms")
            .or_else(|| u32_field(&h.payload, "duration_ms"))
            .or_else(|| u32_field(&h.payload, "permission_wait_ms")),
        ttft_ms: None,
        retry_count: None,
        context_used_tokens: u32_field(&h.payload, "context_used_tokens"),
        context_max_tokens: u32_field(&h.payload, "context_max_tokens"),
        cache_creation_tokens: u32_field(&h.payload, "cache_creation_tokens"),
        cache_read_tokens: u32_field(&h.payload, "cache_read_tokens"),
        system_prompt_tokens: u32_field(&h.payload, "system_prompt_tokens"),
        payload: payload_with_lifecycle(h, lifecycle),
    }
}

fn core_event_kind(kind: &HookKind) -> EventKind {
    match kind {
        HookKind::PreToolUse => EventKind::ToolCall,
        HookKind::PostToolUse => EventKind::ToolResult,
        HookKind::Unknown(_) => EventKind::Hook,
        _ => EventKind::Lifecycle,
    }
}

fn hook_tool(payload: &serde_json::Value) -> Option<String> {
    ["tool_name", "tool"]
        .iter()
        .find_map(|key| payload.get(key).and_then(|value| value.as_str()))
        .map(ToOwned::to_owned)
}

fn hook_tool_id(payload: &serde_json::Value) -> Option<String> {
    ["tool_call_id", "tool_use_id", "call_id", "id"]
        .iter()
        .find_map(|k| payload.get(k).and_then(|v| v.as_str()))
        .map(ToOwned::to_owned)
}

fn lifecycle_type(kind: &HookKind) -> Option<&'static str> {
    match kind {
        HookKind::SessionStart => Some("session_start"),
        HookKind::Stop => Some("session_stop"),
        HookKind::PermissionRequest => Some("permission_request"),
        HookKind::UserPromptSubmit => Some("user_prompt_submit"),
        HookKind::Notification => Some("notification"),
        HookKind::SubagentStart => Some("subagent_start"),
        HookKind::SubagentStop => Some("subagent_stop"),
        HookKind::Interrupt => Some("interrupt"),
        HookKind::ModeTransition => Some("mode_transition"),
        _ => None,
    }
}

fn payload_with_lifecycle(h: &HookEvent, typ: Option<&str>) -> serde_json::Value {
    let mut payload = h.payload.clone();
    if let Some(typ) = typ {
        payload["type"] = serde_json::json!(typ);
    }
    payload
}

fn u32_field(payload: &serde_json::Value, key: &str) -> Option<u32> {
    payload
        .get(key)?
        .as_u64()
        .and_then(|n| u32::try_from(n).ok())
}

fn text_field(payload: &serde_json::Value, key: &str) -> Option<String> {
    payload
        .get(key)
        .and_then(|v| v.as_str())
        .map(ToOwned::to_owned)
}

/// Derive target SessionStatus from hook kind.
/// PreToolUse → Waiting, PostToolUse → Running, Stop → Done,
/// SessionStart → Running, Unknown → None.
pub fn hook_to_status(kind: &HookKind) -> Option<SessionStatus> {
    match kind {
        HookKind::PreToolUse => Some(SessionStatus::Waiting),
        HookKind::PostToolUse => Some(SessionStatus::Running),
        HookKind::Stop => Some(SessionStatus::Done),
        HookKind::SessionStart => Some(SessionStatus::Running),
        HookKind::PermissionRequest => Some(SessionStatus::Waiting),
        HookKind::UserPromptSubmit => Some(SessionStatus::Running),
        HookKind::Notification
        | HookKind::SubagentStart
        | HookKind::SubagentStop
        | HookKind::Interrupt
        | HookKind::ModeTransition => None,
        HookKind::Unknown(_) => None,
    }
}

#[cfg(test)]
#[path = "normalize_tests.rs"]
mod tests;
