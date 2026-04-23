// SPDX-License-Identifier: AGPL-3.0-or-later
use quint_connect::*;
use serde::Deserialize;

#[derive(Debug, Eq, PartialEq, Deserialize)]
struct GcState {
    #[serde(rename = "keepDays")]
    keep_days: i32,
    refused: bool,
    pruned: bool,
    vacuumed: bool,
}

#[derive(Debug)]
struct GcDriver {
    keep_days: i32,
    refused: bool,
    pruned: bool,
    vacuumed: bool,
}

impl Default for GcDriver {
    fn default() -> Self {
        Self {
            keep_days: 7,
            refused: false,
            pruned: false,
            vacuumed: false,
        }
    }
}

impl State<GcDriver> for GcState {
    fn from_driver(d: &GcDriver) -> Result<Self> {
        Ok(GcState {
            keep_days: d.keep_days,
            refused: d.refused,
            pruned: d.pruned,
            vacuumed: d.vacuumed,
        })
    }
}

impl Driver for GcDriver {
    type State = GcState;

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => {
                self.keep_days = 7;
                self.refused = false;
                self.pruned = false;
                self.vacuumed = false;
            },
            step => {
                self.keep_days = 7;
                self.refused = false;
                self.pruned = false;
                self.vacuumed = false;
            },
            set_keep_zero => {
                self.keep_days = 0;
            },
            set_keep_positive => {
                self.keep_days = 30;
            },
            try_prune => {
                if self.keep_days == 0 {
                    self.refused = true;
                    self.pruned = false;
                } else {
                    self.refused = false;
                    self.pruned = true;
                }
            },
            run_vacuum => {
                if !self.pruned {
                    anyhow::bail!("run_vacuum not enabled");
                }
                self.vacuumed = true;
            },
        })
    }
}

#[quint_run(
    spec = "specs/gc-prune.qnt",
    max_samples = 16,
    max_steps = 10,
    seed = "0x3"
)]
fn gc_prune_run() -> impl Driver {
    GcDriver::default()
}
