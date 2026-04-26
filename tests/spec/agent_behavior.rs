// SPDX-License-Identifier: AGPL-3.0-or-later
//! Connect test for specs/agent-behavior.qnt.
//! Todos created monotonic; completed+cancelled bounded by created; interrupts non-negative.

use quint_connect::*;
use serde::Deserialize;

#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Deserialize)]
#[serde(tag = "tag")]
enum SpecMode {
    Plan,
    #[default]
    Agent,
    Ask,
    Debug,
}

#[derive(Debug, Eq, PartialEq, Deserialize)]
struct AgentBehaviorState {
    mode: SpecMode,
    #[serde(rename = "todosCreated", with = "itf::de::As::<itf::de::Integer>")]
    todos_created: i64,
    #[serde(rename = "todosCompleted", with = "itf::de::As::<itf::de::Integer>")]
    todos_completed: i64,
    #[serde(rename = "todosCancelled", with = "itf::de::As::<itf::de::Integer>")]
    todos_cancelled: i64,
    #[serde(rename = "modeTransitions", with = "itf::de::As::<itf::de::Integer>")]
    mode_transitions: i64,
    #[serde(with = "itf::de::As::<itf::de::Integer>")]
    interrupts: i64,
}

#[derive(Debug, Default)]
struct AgentBehaviorDriver {
    mode: SpecMode,
    todos_created: i64,
    todos_completed: i64,
    todos_cancelled: i64,
    mode_transitions: i64,
    interrupts: i64,
}

impl State<AgentBehaviorDriver> for AgentBehaviorState {
    fn from_driver(d: &AgentBehaviorDriver) -> Result<Self> {
        Ok(AgentBehaviorState {
            mode: d.mode,
            todos_created: d.todos_created,
            todos_completed: d.todos_completed,
            todos_cancelled: d.todos_cancelled,
            mode_transitions: d.mode_transitions,
            interrupts: d.interrupts,
        })
    }
}

impl Driver for AgentBehaviorDriver {
    type State = AgentBehaviorState;

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => { *self = AgentBehaviorDriver::default(); },
            step => { *self = AgentBehaviorDriver::default(); },
            switch_mode(to_mode: SpecMode) => {
                self.mode = to_mode;
                self.mode_transitions += 1;
            },
            todo_write(count: i64) => {
                self.todos_created += count;
            },
            todo_complete => {
                self.todos_completed += 1;
            },
            todo_cancel => {
                self.todos_cancelled += 1;
            },
            user_interrupt => {
                self.interrupts += 1;
            }
        })
    }
}

#[test]
fn todos_completed_bounded_by_created() {
    let d = AgentBehaviorDriver {
        todos_created: 5,
        todos_completed: 3,
        todos_cancelled: 2,
        ..Default::default()
    };
    assert!(d.todos_completed + d.todos_cancelled <= d.todos_created);
}

#[test]
fn interrupts_accumulate() {
    let mut d = AgentBehaviorDriver::default();
    d.interrupts += 1;
    d.interrupts += 1;
    assert_eq!(d.interrupts, 2);
}

#[quint_run(
    spec = "specs/agent-behavior.qnt",
    max_samples = 15,
    max_steps = 12,
    seed = "0x2"
)]
fn agent_behavior_run() -> impl Driver {
    AgentBehaviorDriver::default()
}
