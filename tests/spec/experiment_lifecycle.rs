// SPDX-License-Identifier: AGPL-3.0-or-later
use kaizen::experiment::types::{State as ExpLifecycle, transition};
use quint_connect::*;
use serde::Deserialize;

#[derive(Debug, Eq, PartialEq, Deserialize)]
#[serde(tag = "tag")]
enum SpecState {
    Draft,
    Running,
    Concluded,
    Archived,
}

#[derive(Debug, Eq, PartialEq, Deserialize)]
struct ExpState {
    state: SpecState,
}

impl State<ExpDriver> for ExpState {
    fn from_driver(d: &ExpDriver) -> Result<Self> {
        Ok(ExpState {
            state: match d.state {
                ExpLifecycle::Draft => SpecState::Draft,
                ExpLifecycle::Running => SpecState::Running,
                ExpLifecycle::Concluded => SpecState::Concluded,
                ExpLifecycle::Archived => SpecState::Archived,
            },
        })
    }
}

#[derive(Debug)]
struct ExpDriver {
    state: ExpLifecycle,
}

impl Default for ExpDriver {
    fn default() -> Self {
        Self {
            state: ExpLifecycle::Draft,
        }
    }
}

impl Driver for ExpDriver {
    type State = ExpState;

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => {
                self.state = ExpLifecycle::Draft;
            },
            // See `specs/session-lifecycle` driver: outer `any{}` can emit a `step` before the
            // concrete action; keep the same baseline as that pattern.
            step => {
                self.state = ExpLifecycle::Draft;
            },
            start => {
                self.state = transition(self.state, "start")
                    .ok_or_else(|| anyhow::anyhow!("start not enabled"))?;
            },
            conclude => {
                self.state = transition(self.state, "conclude")
                    .ok_or_else(|| anyhow::anyhow!("conclude not enabled"))?;
            },
            archive => {
                self.state = transition(self.state, "archive")
                    .ok_or_else(|| anyhow::anyhow!("archive not enabled"))?;
            }
        })
    }
}

#[quint_run(
    spec = "specs/experiment-lifecycle.qnt",
    max_samples = 20,
    max_steps = 6,
    seed = "0x1"
)]
fn experiment_lifecycle_run() -> impl Driver {
    ExpDriver::default()
}
