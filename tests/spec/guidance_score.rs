// SPDX-License-Identifier: AGPL-3.0-or-later
//! Connect test for `specs/guidance-score.qnt`.

use quint_connect::*;
use serde::Deserialize;

#[derive(Debug, Eq, PartialEq, Deserialize)]
struct ScoreState {
    train_sessions: i64,
    validation_sessions: i64,
    train_score: i64,
    validation_score: i64,
    gate: i64,
}

#[derive(Debug, Default)]
struct ScoreDriver {
    train_sessions: i64,
    validation_sessions: i64,
    train_score: i64,
    validation_score: i64,
    gate: i64,
}

impl State<ScoreDriver> for ScoreState {
    fn from_driver(d: &ScoreDriver) -> Result<Self> {
        Ok(ScoreState {
            train_sessions: d.train_sessions,
            validation_sessions: d.validation_sessions,
            train_score: d.train_score,
            validation_score: d.validation_score,
            gate: d.gate,
        })
    }
}

impl Driver for ScoreDriver {
    type State = ScoreState;

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => { *self = Self::default(); },
            train_high => { self.train_sessions = 20; self.train_score = 90; self.refresh(); },
            validation_low => {
                self.validation_sessions = 10;
                self.validation_score = 60;
                self.refresh();
            },
            validation_high => {
                self.validation_sessions = 10;
                self.validation_score = 88;
                self.refresh();
            },
            validation_tiny => { self.validation_sessions = 3; self.refresh(); },
            step => { self.refresh(); },
        })
    }
}

impl ScoreDriver {
    fn refresh(&mut self) {
        self.gate = gate(
            self.train_sessions,
            self.validation_sessions,
            self.train_score,
            self.validation_score,
        );
    }
}

fn gate(train_sessions: i64, validation_sessions: i64, train_score: i64, val_score: i64) -> i64 {
    match (train_sessions + validation_sessions, validation_sessions) {
        (0, _) => 0,
        (_, n) if train_sessions == 0 || n < 10 => 1,
        _ if val_score + 10 < train_score => 3,
        _ => 2,
    }
}

#[test]
fn regression_requires_held_out_drop() {
    assert_eq!(gate(20, 10, 90, 60), 3);
    assert_eq!(gate(20, 10, 90, 88), 2);
}

#[quint_run(spec = "specs/guidance-score.qnt", max_samples = 20, max_steps = 6)]
fn guidance_score_run() -> impl Driver {
    ScoreDriver::default()
}
