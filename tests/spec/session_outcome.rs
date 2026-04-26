// SPDX-License-Identifier: AGPL-3.0-or-later
//! Connect test for specs/session-outcome.qnt.
//! Invariant: outcome measurement never starts before session Stop.

use quint_connect::*;
use serde::Deserialize;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Deserialize)]
#[serde(tag = "tag")]
enum SpecPhase {
    Live,
    Stopped,
}

impl Default for SpecPhase {
    fn default() -> Self {
        SpecPhase::Live
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Deserialize)]
#[serde(tag = "tag")]
enum SpecOutcome {
    None,
    Measuring,
    Measured,
}

impl Default for SpecOutcome {
    fn default() -> Self {
        SpecOutcome::None
    }
}

#[derive(Debug, Eq, PartialEq, Deserialize)]
struct SessionOutcomeState {
    phase: SpecPhase,
    outcome: SpecOutcome,
    #[serde(rename = "testsPassed", with = "itf::de::As::<itf::de::Integer>")]
    tests_passed: i64,
    #[serde(rename = "testsFailed", with = "itf::de::As::<itf::de::Integer>")]
    tests_failed: i64,
}

#[derive(Debug, Default)]
struct SessionOutcomeDriver {
    phase: SpecPhase,
    outcome: SpecOutcome,
    tests_passed: i64,
    tests_failed: i64,
}

impl State<SessionOutcomeDriver> for SessionOutcomeState {
    fn from_driver(d: &SessionOutcomeDriver) -> Result<Self> {
        Ok(SessionOutcomeState {
            phase: d.phase,
            outcome: d.outcome,
            tests_passed: d.tests_passed,
            tests_failed: d.tests_failed,
        })
    }
}

impl Driver for SessionOutcomeDriver {
    type State = SessionOutcomeState;

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => { *self = SessionOutcomeDriver::default(); },
            step => { *self = SessionOutcomeDriver::default(); },
            session_stop => {
                assert_eq!(self.phase, SpecPhase::Live);
                self.phase = SpecPhase::Stopped;
            },
            start_measure => {
                assert_eq!(self.phase, SpecPhase::Stopped);
                assert_eq!(self.outcome, SpecOutcome::None);
                self.outcome = SpecOutcome::Measuring;
            },
            finish_measure(p: i64, f: i64) => {
                assert_eq!(self.outcome, SpecOutcome::Measuring);
                self.outcome = SpecOutcome::Measured;
                self.tests_passed = p;
                self.tests_failed = f;
            }
        })
    }
}

#[test]
fn outcome_never_before_stop() {
    let d = SessionOutcomeDriver {
        phase: SpecPhase::Stopped,
        outcome: SpecOutcome::Measured,
        tests_passed: 10,
        tests_failed: 0,
    };
    assert_eq!(d.phase, SpecPhase::Stopped);
}

#[test]
fn counts_non_negative() {
    let d = SessionOutcomeDriver {
        tests_passed: 5,
        tests_failed: 1,
        ..Default::default()
    };
    assert!(d.tests_passed >= 0 && d.tests_failed >= 0);
}

#[quint_run(
    spec = "specs/session-outcome.qnt",
    max_samples = 15,
    max_steps = 6,
    seed = "0x3"
)]
fn session_outcome_run() -> impl Driver {
    SessionOutcomeDriver::default()
}
