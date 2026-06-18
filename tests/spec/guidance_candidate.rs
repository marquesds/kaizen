// SPDX-License-Identifier: AGPL-3.0-or-later
use quint_connect::*;
use serde::Deserialize;

#[derive(Debug, Eq, PartialEq, Deserialize)]
struct CandidateState {
    state: i64,
    mutated_artifacts: i64,
}

#[derive(Debug, Default)]
struct CandidateDriver {
    state: i64,
    mutated_artifacts: i64,
}

impl State<CandidateDriver> for CandidateState {
    fn from_driver(driver: &CandidateDriver) -> Result<Self> {
        Ok(Self {
            state: driver.state,
            mutated_artifacts: driver.mutated_artifacts,
        })
    }
}

impl Driver for CandidateDriver {
    type State = CandidateState;

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => { *self = Self::default(); },
            step => { self.advance(); },
            propose => { if self.state == 0 { self.state = 1; } },
            reject => { if self.state == 1 { self.state = 2; } },
            archive => { if self.state == 1 || self.state == 2 { self.state = 3; } },
        })
    }
}

impl CandidateDriver {
    fn advance(&mut self) {
        self.state = match self.state {
            0 => 1,
            1 => 2,
            2 => 3,
            state => state,
        };
    }
}

#[test]
fn proposal_never_mutates_target_artifact() {
    let driver = CandidateDriver {
        state: 1,
        ..Default::default()
    };
    assert_eq!(driver.mutated_artifacts, 0);
}

#[quint_run(spec = "specs/guidance-candidate.qnt", max_samples = 20, max_steps = 8)]
fn guidance_candidate_run() -> impl Driver {
    CandidateDriver::default()
}
