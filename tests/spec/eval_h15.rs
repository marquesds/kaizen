// SPDX-License-Identifier: AGPL-3.0-or-later
use quint_connect::*;
use serde::Deserialize;

#[derive(Debug, Eq, PartialEq, Deserialize)]
struct EvalH15State {
    n_evals: i64,
    n_low: i64,
    sum_scores: i64,
    prior_mean: i64,
    h15_fired: bool,
}

#[derive(Debug, Default)]
struct EvalH15Driver {
    n_evals: i64,
    n_low: i64,
    sum_scores: i64,
    prior_mean: i64,
    h15_fired: bool,
}

fn trigger(n_l: i64, n_e: i64, s: i64, pm: i64) -> bool {
    n_l >= 3 || (n_e > 0 && pm > 0 && (pm * n_e - s) > 15 * n_e)
}

impl State<EvalH15Driver> for EvalH15State {
    fn from_driver(d: &EvalH15Driver) -> Result<Self> {
        Ok(EvalH15State {
            n_evals: d.n_evals,
            n_low: d.n_low,
            sum_scores: d.sum_scores,
            prior_mean: d.prior_mean,
            h15_fired: d.h15_fired,
        })
    }
}

impl Driver for EvalH15Driver {
    type State = EvalH15State;

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => {
                self.n_evals = 0;
                self.n_low = 0;
                self.sum_scores = 0;
                self.prior_mean = 0;
                self.h15_fired = false;
            },
            step => {},
            add_low => {
                let fired = self.h15_fired
                    || trigger(self.n_low + 1, self.n_evals + 1, self.sum_scores + 20, self.prior_mean);
                self.n_evals += 1;
                self.n_low += 1;
                self.sum_scores += 20;
                self.h15_fired = fired;
            },
            add_ok => {
                let fired = self.h15_fired
                    || trigger(self.n_low, self.n_evals + 1, self.sum_scores + 70, self.prior_mean);
                self.n_evals += 1;
                self.sum_scores += 70;
                self.h15_fired = fired;
            },
            rotate => {
                if self.n_evals > 0 {
                    self.prior_mean = self.sum_scores / self.n_evals;
                    self.n_evals = 0;
                    self.n_low = 0;
                    self.sum_scores = 0;
                    self.h15_fired = false;
                }
            },
        })
    }
}

#[quint_run(spec = "specs/eval-h15.qnt", max_samples = 20, max_steps = 12)]
fn eval_h15_run() -> impl Driver {
    EvalH15Driver::default()
}
