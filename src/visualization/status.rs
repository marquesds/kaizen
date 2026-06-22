use super::DerivedStatus;
use crate::core::event::{SessionRecord, SessionStatus};

const ACTIVE_TTL_MS: u64 = 5 * 60_000;
const ORPHAN_TTL_MS: u64 = 30 * 60_000;

pub(crate) fn derive_status(
    session: &SessionRecord,
    last_event_ms: Option<u64>,
    error_count: u64,
    now_ms: u64,
) -> (DerivedStatus, String) {
    if error_count > 0 {
        return (DerivedStatus::Errored, "error event".into());
    }
    if session.status == SessionStatus::Done || session.ended_at_ms.is_some() {
        return (DerivedStatus::Done, "session ended".into());
    }
    stale_status(last_event_ms, now_ms)
}

fn stale_status(last_event_ms: Option<u64>, now_ms: u64) -> (DerivedStatus, String) {
    match last_event_ms {
        Some(ts) if now_ms.saturating_sub(ts) <= ACTIVE_TTL_MS => {
            (DerivedStatus::Active, "recent event".into())
        }
        Some(ts) if now_ms.saturating_sub(ts) >= ORPHAN_TTL_MS => {
            (DerivedStatus::Orphaned, "stale open session".into())
        }
        Some(_) => (DerivedStatus::Idle, "no recent event".into()),
        None => (DerivedStatus::Idle, "no events".into()),
    }
}
