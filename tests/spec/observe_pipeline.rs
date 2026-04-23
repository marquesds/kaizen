// SPDX-License-Identifier: AGPL-3.0-or-later
use quint_connect::*;
use serde::Deserialize;

#[derive(Debug, Eq, PartialEq, Deserialize)]
struct ObserveState {
    phase: String,
}

#[derive(Debug)]
struct ObserveDriver {
    phase: String,
}

impl Default for ObserveDriver {
    fn default() -> Self {
        Self {
            phase: "Idle".into(),
        }
    }
}

impl State<ObserveDriver> for ObserveState {
    fn from_driver(d: &ObserveDriver) -> Result<Self> {
        Ok(ObserveState {
            phase: d.phase.clone(),
        })
    }
}

impl Driver for ObserveDriver {
    type State = ObserveState;

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => {
                self.phase = "Idle".into();
            },
            step => {
                self.phase = "Idle".into();
            },
            resolve_workspace => {
                if self.phase != "Idle" {
                    anyhow::bail!("resolve_workspace not enabled");
                }
                self.phase = "Resolved".into();
            },
            load_config => {
                if self.phase != "Resolved" {
                    anyhow::bail!("load_config not enabled");
                }
                self.phase = "ConfigLoaded".into();
            },
            open_store => {
                if self.phase != "ConfigLoaded" {
                    anyhow::bail!("open_store not enabled");
                }
                self.phase = "StoreOpen".into();
            },
            scan_agents => {
                if self.phase != "StoreOpen" {
                    anyhow::bail!("scan_agents not enabled");
                }
                self.phase = "Scanned".into();
            },
            run_query => {
                if self.phase != "Scanned" {
                    anyhow::bail!("run_query not enabled");
                }
                self.phase = "Queried".into();
            },
            emit_output => {
                if self.phase != "Queried" {
                    anyhow::bail!("emit_output not enabled");
                }
                self.phase = "Done".into();
            },
        })
    }
}

#[quint_run(
    spec = "specs/observe-pipeline.qnt",
    max_samples = 14,
    max_steps = 10,
    seed = "0x4"
)]
fn observe_pipeline_run() -> impl Driver {
    ObserveDriver::default()
}
