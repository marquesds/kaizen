// SPDX-License-Identifier: AGPL-3.0-or-later
use quint_connect::*;
use serde::Deserialize;

#[derive(Debug, Eq, PartialEq, Deserialize)]
struct H33AutomationState {
    #[serde(rename = "distinct_qualifying_sessions")]
    distinct_qualifying_sessions: i64,
    #[serde(rename = "max_qualifying_run_len")]
    max_qualifying_run_len: i64,
}

#[derive(Debug, Default)]
struct H33AutomationDriver {
    distinct_qualifying_sessions: i64,
    max_qualifying_run_len: i64,
}

impl State<H33AutomationDriver> for H33AutomationState {
    fn from_driver(d: &H33AutomationDriver) -> Result<Self> {
        Ok(H33AutomationState {
            distinct_qualifying_sessions: d.distinct_qualifying_sessions,
            max_qualifying_run_len: d.max_qualifying_run_len,
        })
    }
}

impl Driver for H33AutomationDriver {
    type State = H33AutomationState;

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => {
                self.distinct_qualifying_sessions = 0;
                self.max_qualifying_run_len = 0;
            },
            step => {},
            set_one_session_run5 => {
                self.distinct_qualifying_sessions = 1;
                self.max_qualifying_run_len = 5;
            },
            set_one_session_run10 => {
                self.distinct_qualifying_sessions = 1;
                self.max_qualifying_run_len = 10;
            },
            set_two_sessions_run5 => {
                self.distinct_qualifying_sessions = 2;
                self.max_qualifying_run_len = 5;
            },
            set_no_qualifying => {
                self.distinct_qualifying_sessions = 0;
                self.max_qualifying_run_len = 4;
            },
        })
    }
}

#[quint_run(spec = "specs/h33-automation.qnt", max_samples = 20, max_steps = 12)]
fn h33_automation_run() -> impl Driver {
    H33AutomationDriver::default()
}
