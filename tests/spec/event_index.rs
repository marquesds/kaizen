// SPDX-License-Identifier: AGPL-3.0-or-later
//! Simulation for `specs/event-index.qnt` (slug grammar for derived skills/rules rows).
//! State is checked by the spec only; the driver is a no-op placeholder for trace replay.

use quint_connect::*;

struct EventIndexDriver;

impl Driver for EventIndexDriver {
    type State = ();

    fn step(&mut self, _step: &Step) -> Result {
        Ok(())
    }
}

#[quint_run(spec = "specs/event-index.qnt", max_samples = 20, max_steps = 8)]
fn event_index_run() -> impl Driver {
    EventIndexDriver
}
