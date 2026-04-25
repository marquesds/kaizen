// SPDX-License-Identifier: AGPL-3.0-or-later
use quint_connect::*;
use serde::Deserialize;

#[derive(Debug, Eq, PartialEq, Deserialize)]
struct PromptTrackingState {
    session_state: i64,
    stored_fingerprint: i64,
    stop_fingerprint: i64,
    prompt_changed_fired: bool,
}

#[derive(Debug, Default)]
struct PromptTrackingDriver {
    session_state: i64,
    stored_fingerprint: i64,
    stop_fingerprint: i64,
    prompt_changed_fired: bool,
}

impl State<PromptTrackingDriver> for PromptTrackingState {
    fn from_driver(d: &PromptTrackingDriver) -> Result<Self> {
        Ok(PromptTrackingState {
            session_state: d.session_state,
            stored_fingerprint: d.stored_fingerprint,
            stop_fingerprint: d.stop_fingerprint,
            prompt_changed_fired: d.prompt_changed_fired,
        })
    }
}

impl Driver for PromptTrackingDriver {
    type State = PromptTrackingState;

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => {
                self.session_state = 0;
                self.stored_fingerprint = 0;
                self.stop_fingerprint = 0;
                self.prompt_changed_fired = false;
            },
            step => {},
            start_a => {
                if self.session_state == 0 {
                    self.session_state = 1;
                    self.stored_fingerprint = 1;
                    self.prompt_changed_fired = false;
                }
            },
            start_b => {
                if self.session_state == 0 {
                    self.session_state = 1;
                    self.stored_fingerprint = 2;
                    self.prompt_changed_fired = false;
                }
            },
            stop_a => {
                if self.session_state == 1 {
                    self.prompt_changed_fired = 1 != self.stored_fingerprint;
                    self.session_state = 0;
                    self.stop_fingerprint = 1;
                }
            },
            stop_b => {
                if self.session_state == 1 {
                    self.prompt_changed_fired = 2 != self.stored_fingerprint;
                    self.session_state = 0;
                    self.stop_fingerprint = 2;
                }
            },
        })
    }
}

#[quint_run(spec = "specs/prompt-tracking.qnt", max_samples = 20, max_steps = 12)]
fn prompt_tracking_run() -> impl Driver {
    PromptTrackingDriver::default()
}
