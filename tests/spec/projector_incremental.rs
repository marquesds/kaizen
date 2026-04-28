// SPDX-License-Identifier: AGPL-3.0-or-later
use quint_connect::*;

struct ProjectorIncrementalDriver;

impl Driver for ProjectorIncrementalDriver {
    type State = ();

    fn step(&mut self, _step: &Step) -> Result {
        Ok(())
    }
}

#[quint_run(
    spec = "specs/projector-incremental.qnt",
    max_samples = 20,
    max_steps = 8
)]
fn projector_incremental_run() -> impl Driver {
    ProjectorIncrementalDriver
}
