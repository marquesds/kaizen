// SPDX-License-Identifier: AGPL-3.0-or-later
use quint_connect::*;
use serde::Deserialize;

#[derive(Debug, Eq, PartialEq, Deserialize)]
struct RetroPipelineState {
    phase: String,
    #[serde(rename = "lockHeld")]
    lock_held: bool,
}

#[derive(Debug, Default)]
struct RetroDriver {
    phase: String,
    lock_held: bool,
}

impl State<RetroDriver> for RetroPipelineState {
    fn from_driver(d: &RetroDriver) -> Result<Self> {
        Ok(RetroPipelineState {
            phase: d.phase.clone(),
            lock_held: d.lock_held,
        })
    }
}

impl Driver for RetroDriver {
    type State = RetroPipelineState;

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => {
                self.phase = "Idle".into();
                self.lock_held = false;
            },
            step => {
                self.phase = "Idle".into();
                self.lock_held = false;
            },
            acquire => {
                if self.phase == "Idle" && !self.lock_held {
                    self.lock_held = true;
                    self.phase = "Loading".into();
                }
            },
            load_done => {
                if self.phase == "Loading" {
                    self.phase = "Computing".into();
                }
            },
            compute_done => {
                if self.phase == "Computing" {
                    self.phase = "Ranking".into();
                }
            },
            rank_done => {
                if self.phase == "Ranking" {
                    self.phase = "Writing".into();
                }
            },
            write_done => {
                if self.phase == "Writing" {
                    self.lock_held = false;
                    self.phase = "Idle".into();
                }
            },
        })
    }
}

#[quint_run(spec = "specs/retro-pipeline.qnt", max_samples = 10, max_steps = 8)]
fn retro_pipeline_run() -> impl Driver {
    RetroDriver::default()
}
