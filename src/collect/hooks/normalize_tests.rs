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
fn stop_maps_to_lifecycle_event() {
    let ev = hook_to_event(&make_event(HookKind::Stop), 5);
    assert_eq!((ev.kind, ev.seq), (EventKind::Lifecycle, 5));
    assert_eq!(ev.payload["type"], "session_stop");
}

#[test]
fn pre_tool_hook_maps_to_named_tool_call() {
    let ev = named_tool_event(HookKind::PreToolUse, 0);
    assert_eq!(ev.kind, EventKind::ToolCall);
    assert_eq!(ev.tool.as_deref(), Some("Read"));
}

#[test]
fn post_tool_hook_maps_to_named_tool_result() {
    let ev = named_tool_event(HookKind::PostToolUse, 1);
    assert_eq!(ev.kind, EventKind::ToolResult);
    assert_eq!(ev.tool.as_deref(), Some("Read"));
}

fn named_tool_event(kind: HookKind, seq: u64) -> Event {
    let mut hook = make_event(kind);
    hook.payload = json!({"tool_name":"Read","tool_use_id":"call-1"});
    hook_to_event(&hook, seq)
}

#[test]
fn hook_to_event_maps_total_cost_usd_to_microdollars() {
    let mut hook = make_event(HookKind::Stop);
    hook.payload = json!({"total_cost_usd":0.042});
    assert_eq!(hook_to_event(&hook, 0).cost_usd_e6, Some(42_000));
}

#[test]
fn permission_request_maps_lifecycle_wait() {
    let mut hook = make_event(HookKind::PermissionRequest);
    hook.payload = json!({"permission_wait_ms":250});
    let event = hook_to_event(&hook, 0);
    assert_eq!(
        (event.kind, event.latency_ms),
        (EventKind::Lifecycle, Some(250))
    );
    assert_eq!(event.payload["type"], "permission_request");
    assert_eq!(hook_to_status(&hook.kind), Some(SessionStatus::Waiting));
}
