// SPDX-License-Identifier: AGPL-3.0-or-later
use quint_connect::*;
use serde::Deserialize;

#[derive(Debug, Eq, PartialEq, Deserialize)]
struct MetricsPipeState {
    phase: String,
}

#[derive(Debug)]
struct MetricsPipeDriver {
    phase: String,
}

impl Default for MetricsPipeDriver {
    fn default() -> Self {
        Self {
            phase: "Idle".into(),
        }
    }
}

impl State<MetricsPipeDriver> for MetricsPipeState {
    fn from_driver(d: &MetricsPipeDriver) -> Result<Self> {
        Ok(MetricsPipeState {
            phase: d.phase.clone(),
        })
    }
}

impl Driver for MetricsPipeDriver {
    type State = MetricsPipeState;

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
            ensure_index => {
                if self.phase != "Scanned" {
                    anyhow::bail!("ensure_index not enabled");
                }
                self.phase = "Indexed".into();
            },
            build_report => {
                if self.phase != "Indexed" {
                    anyhow::bail!("build_report not enabled");
                }
                self.phase = "Reported".into();
            },
            emit_output => {
                if self.phase != "Reported" {
                    anyhow::bail!("emit_output not enabled");
                }
                self.phase = "Done".into();
            },
        })
    }
}

#[quint_run(
    spec = "specs/metrics-pipeline.qnt",
    max_samples = 14,
    max_steps = 12,
    seed = "0x6"
)]
fn metrics_pipeline_run() -> impl Driver {
    MetricsPipeDriver::default()
}
