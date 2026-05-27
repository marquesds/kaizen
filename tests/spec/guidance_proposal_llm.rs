// SPDX-License-Identifier: AGPL-3.0-or-later
//! Connect test for `specs/guidance-proposal-llm.qnt`.

use quint_connect::*;
use serde::Deserialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize)]
struct ProposalState {
    config_enabled: bool,
    llm_flag: bool,
    redact_config: bool,
    raw_input_seen: bool,
    sent_raw_input: bool,
    rejected_memory_seen: bool,
    sent_rejected_memory: bool,
    requested_ops: i64,
    emitted_ops: i64,
    candidate_generated: bool,
    error: bool,
}

#[derive(Debug)]
struct ProposalDriver {
    state: ProposalState,
}

impl Default for ProposalDriver {
    fn default() -> Self {
        Self {
            state: ProposalState {
                config_enabled: false,
                llm_flag: false,
                redact_config: true,
                raw_input_seen: false,
                sent_raw_input: false,
                rejected_memory_seen: false,
                sent_rejected_memory: false,
                requested_ops: 0,
                emitted_ops: 0,
                candidate_generated: false,
                error: false,
            },
        }
    }
}

impl State<ProposalDriver> for ProposalState {
    fn from_driver(d: &ProposalDriver) -> Result<Self> {
        Ok(d.state)
    }
}

impl Driver for ProposalDriver {
    type State = ProposalState;

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => { *self = Self::default(); },
            enable_config => { self.state.config_enabled = true; },
            request_llm => { self.state.llm_flag = true; },
            attach_inputs => {
                self.state.raw_input_seen = true;
                self.state.rejected_memory_seen = true;
                self.state.requested_ops = 5;
            },
            disable_redaction => { self.state.redact_config = false; },
            generate => { self.generate(); },
            step => {},
        })
    }
}

impl ProposalDriver {
    fn generate(&mut self) {
        let allowed = self.state.config_enabled && self.state.llm_flag;
        self.state.sent_raw_input = self.state.raw_input_seen && !self.state.redact_config;
        self.state.sent_rejected_memory = self.state.rejected_memory_seen;
        self.state.emitted_ops = if allowed {
            self.state.requested_ops.min(3)
        } else {
            0
        };
        self.state.candidate_generated = allowed;
        self.state.error = !allowed;
    }
}

#[test]
fn generation_requires_flag_and_config() {
    let mut d = ProposalDriver::default();
    d.state.llm_flag = true;
    d.generate();
    assert!(d.state.error);
    assert!(!d.state.candidate_generated);
}

#[quint_run(
    spec = "specs/guidance-proposal-llm.qnt",
    max_samples = 20,
    max_steps = 8
)]
fn guidance_proposal_llm_run() -> impl Driver {
    ProposalDriver::default()
}
