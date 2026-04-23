// SPDX-License-Identifier: AGPL-3.0-or-later
//! Session lifecycle state machine — pure core, no IO.
//!
//! # Invariants
//! - `Done` is terminal: `transition` always returns `None` for Done sessions.
//! - `gc` fires only when `elapsed >= GC_DEADLINE`.

pub const GC_DEADLINE: u32 = 24;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Running,
    Waiting,
    Idle,
    Done,
}

pub struct Session {
    pub status: Status,
    pub elapsed: u32,
}

impl Default for Session {
    fn default() -> Self {
        Session {
            status: Status::Running,
            elapsed: 0,
        }
    }
}

/// Pure transition: returns next `Session` or `None` if action is not enabled.
pub fn transition(s: &Session, action: &str) -> Option<Session> {
    let next = |status| Session {
        status,
        elapsed: s.elapsed + 1,
    };
    match (action, &s.status) {
        ("tool_call", Status::Running) => Some(next(Status::Waiting)),
        ("user_responds", Status::Waiting) => Some(next(Status::Running)),
        ("agent_idles", Status::Running) => Some(next(Status::Idle)),
        ("agent_resumes", Status::Idle) => Some(next(Status::Running)),
        ("session_ends", st) if *st != Status::Done => Some(next(Status::Done)),
        ("gc", st) if *st != Status::Done && s.elapsed >= GC_DEADLINE => Some(next(Status::Done)),
        _ => None,
    }
}
