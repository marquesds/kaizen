// SPDX-License-Identifier: AGPL-3.0-or-later
use quint_connect::*;
use serde::Deserialize;

#[derive(Debug, Eq, PartialEq, Deserialize)]
struct LookupState {
    phase: String,
    found: bool,
}

#[derive(Debug)]
struct LookupDriver {
    phase: String,
    found: bool,
}

impl Default for LookupDriver {
    fn default() -> Self {
        Self {
            phase: "Start".into(),
            found: false,
        }
    }
}

impl State<LookupDriver> for LookupState {
    fn from_driver(d: &LookupDriver) -> Result<Self> {
        Ok(LookupState {
            phase: d.phase.clone(),
            found: d.found,
        })
    }
}

impl Driver for LookupDriver {
    type State = LookupState;

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => {
                self.phase = "Start".into();
                self.found = false;
            },
            step => {
                self.phase = "Start".into();
                self.found = false;
            },
            open_store => {
                if self.phase != "Start" {
                    anyhow::bail!("open_store not enabled");
                }
                self.phase = "Open".into();
                self.found = false;
            },
            lookup_hit => {
                if self.phase != "Open" {
                    anyhow::bail!("lookup_hit not enabled");
                }
                self.phase = "Ready".into();
                self.found = true;
            },
            lookup_miss => {
                if self.phase != "Open" {
                    anyhow::bail!("lookup_miss not enabled");
                }
                self.phase = "Missing".into();
                self.found = false;
            },
            render => {
                if self.phase != "Ready" {
                    anyhow::bail!("render not enabled");
                }
                self.phase = "Done".into();
            },
        })
    }
}

#[quint_run(
    spec = "specs/session-lookup.qnt",
    max_samples = 12,
    max_steps = 8,
    seed = "0x5"
)]
fn session_lookup_run() -> impl Driver {
    LookupDriver::default()
}
