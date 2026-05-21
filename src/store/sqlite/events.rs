use super::*;

pub(super) fn projector_legacy_mode() -> bool {
    std::env::var("KAIZEN_PROJECTOR").is_ok_and(|v| v == "legacy")
}

pub(super) fn is_stop_event(e: &Event) -> bool {
    if !matches!(e.kind, EventKind::Hook) {
        return false;
    }
    e.payload
        .get("event")
        .and_then(|v| v.as_str())
        .or_else(|| e.payload.get("hook_event_name").and_then(|v| v.as_str()))
        == Some("Stop")
}

impl Store {}
