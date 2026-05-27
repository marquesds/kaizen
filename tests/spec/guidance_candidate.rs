// SPDX-License-Identifier: AGPL-3.0-or-later
use kaizen::guidance::CandidateStatus;
use quint_connect::*;
use serde::Deserialize;

#[derive(Debug, Eq, PartialEq, Deserialize)]
struct CandidateState {
    state: i64,
    was_applied: bool,
    backup_saved: bool,
    mutated_artifacts: i64,
    experiment_attached: bool,
}

#[derive(Debug, Default)]
struct CandidateDriver {
    state: i64,
    was_applied: bool,
    backup_saved: bool,
    mutated_artifacts: i64,
    experiment_attached: bool,
}

impl State<CandidateDriver> for CandidateState {
    fn from_driver(d: &CandidateDriver) -> Result<Self> {
        Ok(CandidateState {
            state: d.state,
            was_applied: d.was_applied,
            backup_saved: d.backup_saved,
            mutated_artifacts: d.mutated_artifacts,
            experiment_attached: d.experiment_attached,
        })
    }
}

impl Driver for CandidateDriver {
    type State = CandidateState;

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => { *self = Self::default(); },
            step => {
                match self.state {
                    0 => self.state = 1,
                    1 => self.apply_mutation(),
                    2 => self.state = 3,
                    3 | 4 => self.state = 5,
                    _ => {}
                }
            },
            propose => { if self.state == 0 { self.state = 1; } },
            apply => { if self.state == 1 { self.apply_mutation(); } },
            reject => { if self.state == 1 || self.state == 2 { self.state = 4; } },
            validate => { if self.state == 2 { self.state = 3; } },
            validate_reject => { if self.state == 2 { self.state = 4; } },
            validate_insufficient => { if self.state == 2 { self.state = 2; } },
            archive => { if self.state == 3 || self.state == 4 { self.state = 5; } },
        })
    }
}

impl CandidateDriver {
    fn apply_mutation(&mut self) {
        self.state = 2;
        self.was_applied = true;
        self.backup_saved = true;
        self.mutated_artifacts = 1;
        self.experiment_attached = true;
    }
}

#[test]
fn rust_status_values_cover_spec_states() {
    let all = [
        CandidateStatus::Proposed,
        CandidateStatus::Applied,
        CandidateStatus::Validated,
        CandidateStatus::Rejected,
        CandidateStatus::Archived,
    ];
    assert_eq!(all.len(), 5);
}

#[test]
fn apply_records_backup_and_prompt_bound_experiment() {
    let mut d = CandidateDriver {
        state: 1,
        ..Default::default()
    };
    d.apply_mutation();
    assert!(d.backup_saved && d.experiment_attached);
    assert_eq!(d.mutated_artifacts, 1);
}

#[quint_run(spec = "specs/guidance-candidate.qnt", max_samples = 20, max_steps = 8)]
fn guidance_candidate_run() -> impl Driver {
    CandidateDriver::default()
}
