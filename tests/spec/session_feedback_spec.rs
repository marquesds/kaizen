// SPDX-License-Identifier: AGPL-3.0-or-later
use quint_connect::*;
use serde::Deserialize;

#[derive(Debug, Eq, PartialEq, Deserialize)]
struct SessionFeedbackState {
    n_bad: i64,
    n_scored: i64,
    sum_scores: i64,
    h17_fired: bool,
}

#[derive(Debug, Default)]
struct SessionFeedbackDriver {
    n_bad: i64,
    n_scored: i64,
    sum_scores: i64,
    h17_fired: bool,
}

fn trigger(n_b: i64, n_s: i64, s: i64) -> bool {
    n_b >= 2 || (n_s >= 5 && s * 2 <= n_s * 5)
}

impl State<SessionFeedbackDriver> for SessionFeedbackState {
    fn from_driver(d: &SessionFeedbackDriver) -> Result<Self> {
        Ok(SessionFeedbackState {
            n_bad: d.n_bad,
            n_scored: d.n_scored,
            sum_scores: d.sum_scores,
            h17_fired: d.h17_fired,
        })
    }
}

impl Driver for SessionFeedbackDriver {
    type State = SessionFeedbackState;

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => {
                self.n_bad = 0;
                self.n_scored = 0;
                self.sum_scores = 0;
                self.h17_fired = false;
            },
            step => {},
            add_bad => {
                let fired = self.h17_fired
                    || trigger(self.n_bad + 1, self.n_scored, self.sum_scores);
                self.n_bad += 1;
                self.h17_fired = fired;
            },
            add_good => {},
            add_scored_low => {
                let fired = self.h17_fired
                    || trigger(self.n_bad, self.n_scored + 1, self.sum_scores + 2);
                self.n_scored += 1;
                self.sum_scores += 2;
                self.h17_fired = fired;
            },
            add_scored_high => {
                let fired = self.h17_fired
                    || trigger(self.n_bad, self.n_scored + 1, self.sum_scores + 4);
                self.n_scored += 1;
                self.sum_scores += 4;
                self.h17_fired = fired;
            },
        })
    }
}

#[quint_run(spec = "specs/session-feedback.qnt", max_samples = 20, max_steps = 12)]
fn session_feedback_run() -> impl Driver {
    SessionFeedbackDriver::default()
}
