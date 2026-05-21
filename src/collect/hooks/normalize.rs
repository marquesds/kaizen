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
        kind: lifecycle.map_or(EventKind::Hook, |_| EventKind::Lifecycle),
        source: EventSource::Hook,
        tool: None,
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

fn hook_tool_id(payload: &serde_json::Value) -> Option<String> {
    ["tool_call_id", "tool_use_id", "call_id", "id"]
        .iter()
        .find_map(|k| payload.get(k).and_then(|v| v.as_str()))
        .map(ToOwned::to_owned)
}

fn lifecycle_type(kind: &HookKind) -> Option<&'static str> {
    match kind {
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
mod tests {
    use super::*;
    use crate::collect::hooks::HookEvent;
    use serde_json::json;

    fn make_event(kind: HookKind) -> HookEvent {
        HookEvent {
            kind,
            session_id: "s1".to_string(),
            ts_ms: 1000,
            payload: json!({}),
        }
    }

    #[test]
    fn session_start_maps_running() {
        assert_eq!(
            hook_to_status(&HookKind::SessionStart),
            Some(SessionStatus::Running)
        );
    }

    #[test]
    fn pre_tool_use_maps_waiting() {
        assert_eq!(
            hook_to_status(&HookKind::PreToolUse),
            Some(SessionStatus::Waiting)
        );
    }

    #[test]
    fn post_tool_use_maps_running() {
        assert_eq!(
            hook_to_status(&HookKind::PostToolUse),
            Some(SessionStatus::Running)
        );
    }

    #[test]
    fn stop_maps_done() {
        assert_eq!(hook_to_status(&HookKind::Stop), Some(SessionStatus::Done));
    }

    #[test]
    fn unknown_maps_none() {
        assert_eq!(hook_to_status(&HookKind::Unknown("x".to_string())), None);
    }

    #[test]
    fn hook_to_event_kind_is_hook() {
        let h = make_event(HookKind::Stop);
        let ev = hook_to_event(&h, 5);
        assert_eq!(ev.kind, EventKind::Hook);
        assert_eq!(ev.seq, 5);
        assert_eq!(ev.session_id, "s1");
    }

    #[test]
    fn hook_to_event_maps_total_cost_usd_to_microdollars() {
        let h = HookEvent {
            kind: HookKind::Stop,
            session_id: "s1".to_string(),
            ts_ms: 1000,
            payload: json!({ "total_cost_usd": 0.042 }),
        };
        let ev = hook_to_event(&h, 0);
        assert_eq!(ev.cost_usd_e6, Some(42_000));
    }

    #[test]
    fn permission_request_maps_lifecycle_wait() {
        let h = HookEvent {
            kind: HookKind::PermissionRequest,
            session_id: "s1".to_string(),
            ts_ms: 1000,
            payload: json!({"permission_wait_ms": 250}),
        };
        let ev = hook_to_event(&h, 0);
        assert_eq!(ev.kind, EventKind::Lifecycle);
        assert_eq!(ev.latency_ms, Some(250));
        assert_eq!(ev.payload["type"], "permission_request");
        assert_eq!(
            hook_to_status(&HookKind::PermissionRequest),
            Some(SessionStatus::Waiting)
        );
    }
}
