// SPDX-License-Identifier: AGPL-3.0-or-later
use itf::value::Value;
use kaizen::core::event::SessionStatus;
use quint_connect::*;
use serde::Deserialize;

// --- State (mirrors `specs/hook-ingest.qnt` vars) ---

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
    session_exists: bool,
    #[serde(with = "itf::de::As::<itf::de::Integer>")]
    started_at_ms: i64,
    agent: String,
}

// --- Driver ---

#[derive(Default)]
struct HookDriver {
    status: Option<SessionStatus>,
    session_exists: bool,
    started_at_ms: i64,
    agent: String,
}

impl HookDriver {
    fn reset(&mut self) {
        *self = Self::default();
    }

    /// Session row invariants: positive ts + source agent. Lifecycle may stay unchanged
    /// (`on_other` / unknown hook: `hook_to_status` is `None`).
    fn materialize_session(&mut self, src: String) {
        self.session_exists = true;
        self.started_at_ms = 1;
        self.agent = src;
    }

    fn read_src(step: &Step) -> Result<String> {
        let v = step
            .nondet_picks
            .get("src")
            .ok_or_else(|| anyhow::anyhow!("expected nondet pick `src` for hook action"))?;
        match v {
            Value::String(s) => Ok(s.clone()),
            _ => anyhow::bail!("nondet `src` was not a string: {v:?}"),
        }
    }
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
        Ok(HookState {
            status,
            session_exists: d.session_exists,
            started_at_ms: d.started_at_ms,
            agent: d.agent.clone(),
        })
    }
}

impl Driver for HookDriver {
    type State = HookState;

    fn step(&mut self, step: &Step) -> Result {
        match step.action_taken.as_str() {
            "init" | "step" => self.reset(),
            "on_session_start" => {
                self.status = Some(SessionStatus::Running);
                self.materialize_session(Self::read_src(step)?);
            }
            "on_pre_tool_use" => {
                self.status = Some(SessionStatus::Waiting);
                self.materialize_session(Self::read_src(step)?);
            }
            "on_post_tool_use" => {
                self.status = Some(SessionStatus::Running);
                self.materialize_session(Self::read_src(step)?);
            }
            "on_stop" => {
                self.status = Some(SessionStatus::Done);
                self.materialize_session(Self::read_src(step)?);
            }
            "on_other" => {
                // `HookKind::Unknown` → `hook_to_status` is `None`: preserve `self.status` (aligns
                // with `on_other` in the Quint spec, which may leave lifecycle unchanged).
                self.materialize_session(Self::read_src(step)?);
            }
            other => anyhow::bail!("unknown action {other}"),
        }
        Ok(())
    }
}

// --- Test ---

#[quint_run(spec = "specs/hook-ingest.qnt", max_samples = 20, max_steps = 6)]
fn hook_ingest_run() -> impl Driver {
    HookDriver::default()
}
