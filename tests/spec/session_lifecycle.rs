// SPDX-License-Identifier: AGPL-3.0-or-later
use kaizen::core::session::{Session, Status, transition};
use quint_connect::*;
use serde::Deserialize;

// --- State (mirrors Quint vars) ---

#[derive(Debug, Eq, PartialEq, Deserialize)]
#[serde(tag = "tag")]
enum SpecStatus {
    Running,
    Waiting,
    Idle,
    Done,
}

#[derive(Debug, Eq, PartialEq, Deserialize)]
struct SessionState {
    status: SpecStatus,
    #[serde(with = "itf::de::As::<itf::de::Integer>")]
    elapsed: u32,
}

impl State<SessionDriver> for SessionState {
    fn from_driver(d: &SessionDriver) -> Result<Self> {
        Ok(SessionState {
            status: match d.session.status {
                Status::Running => SpecStatus::Running,
                Status::Waiting => SpecStatus::Waiting,
                Status::Idle => SpecStatus::Idle,
                Status::Done => SpecStatus::Done,
            },
            elapsed: d.session.elapsed,
        })
    }
}

// --- Driver ---

#[derive(Default)]
struct SessionDriver {
    session: Session,
}

impl Driver for SessionDriver {
    type State = SessionState;

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            // quint run (Rust backend) reports "step" for the init state
            init => {
                self.session = Session::default();
            },
            step => {
                self.session = Session::default();
            },
            tool_call => {
                self.session = transition(&self.session, "tool_call")
                    .ok_or_else(|| anyhow::anyhow!("tool_call not enabled"))?;
            },
            user_responds => {
                self.session = transition(&self.session, "user_responds")
                    .ok_or_else(|| anyhow::anyhow!("user_responds not enabled"))?;
            },
            agent_idles => {
                self.session = transition(&self.session, "agent_idles")
                    .ok_or_else(|| anyhow::anyhow!("agent_idles not enabled"))?;
            },
            agent_resumes => {
                self.session = transition(&self.session, "agent_resumes")
                    .ok_or_else(|| anyhow::anyhow!("agent_resumes not enabled"))?;
            },
            session_ends => {
                self.session = transition(&self.session, "session_ends")
                    .ok_or_else(|| anyhow::anyhow!("session_ends not enabled"))?;
            },
            gc => {
                self.session = transition(&self.session, "gc")
                    .ok_or_else(|| anyhow::anyhow!("gc not enabled"))?;
            }
        })
    }
}

// --- Test ---
//
// #[quint_test] requires quint test traces with mbt::actionTaken, which the
// Rust backend (default in quint 0.32.0) doesn't emit. #[quint_run] uses
// quint run --mbt which does emit the field. See spike-b-quint.md.
#[quint_run(spec = "specs/session-lifecycle.qnt", max_samples = 20, max_steps = 6)]
fn session_lifecycle_run() -> impl Driver {
    SessionDriver::default()
}
