// SPDX-License-Identifier: AGPL-3.0-or-later
use kaizen::collect::hooks::EventKind as HookKind;
use kaizen::collect::hooks::normalize::hook_to_status;
use kaizen::core::event::SessionStatus;
use quint_connect::*;
use serde::Deserialize;

// --- State (mirrors Quint vars) ---

#[derive(Debug, Eq, PartialEq, Deserialize)]
#[serde(tag = "tag")]
enum SpecStatus {
    NotStarted,
    Running,
    Waiting,
    Done,
}

#[derive(Debug, Eq, PartialEq, Deserialize)]
struct HookState {
    status: SpecStatus,
}

// --- Driver ---
// None = NotStarted (spec initial); Some(x) = post-hook status.

#[derive(Default)]
struct HookDriver {
    status: Option<SessionStatus>,
}

impl State<HookDriver> for HookState {
    fn from_driver(d: &HookDriver) -> Result<Self> {
        let status = match &d.status {
            None => SpecStatus::NotStarted,
            Some(SessionStatus::Running) => SpecStatus::Running,
            Some(SessionStatus::Waiting) => SpecStatus::Waiting,
            Some(SessionStatus::Done) => SpecStatus::Done,
            Some(SessionStatus::Idle) => SpecStatus::NotStarted,
        };
        Ok(HookState { status })
    }
}

fn apply(driver: &mut HookDriver, kind: &HookKind) -> Result {
    driver.status = Some(
        hook_to_status(kind)
            .unwrap_or_else(|| driver.status.clone().unwrap_or(SessionStatus::Idle)),
    );
    Ok(())
}

impl Driver for HookDriver {
    type State = HookState;

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => { self.status = None; },
            // fallback: reset for unknown combined-step actions
            step => { self.status = None; },
            on_session_start => apply(self, &HookKind::SessionStart)?,
            on_pre_tool_use  => apply(self, &HookKind::PreToolUse)?,
            on_post_tool_use => apply(self, &HookKind::PostToolUse)?,
            on_stop          => apply(self, &HookKind::Stop)?,
            on_other         => {
                // Unknown → no transition; preserve current status
                let _ = hook_to_status(&HookKind::Unknown("x".to_string()));
            }
        })
    }
}

// --- Test ---

#[quint_run(spec = "specs/hook-ingest.qnt", max_samples = 20, max_steps = 6)]
fn hook_ingest_run() -> impl Driver {
    HookDriver::default()
}
