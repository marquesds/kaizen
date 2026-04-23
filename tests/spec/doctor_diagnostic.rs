// SPDX-License-Identifier: AGPL-3.0-or-later
use quint_connect::*;
use serde::Deserialize;

#[derive(Debug, Eq, PartialEq, Deserialize)]
struct DoctorState {
    phase: String,
    failed: bool,
}

#[derive(Debug)]
struct DoctorDriver {
    phase: String,
    failed: bool,
}

impl Default for DoctorDriver {
    fn default() -> Self {
        Self {
            phase: "Start".into(),
            failed: false,
        }
    }
}

impl State<DoctorDriver> for DoctorState {
    fn from_driver(d: &DoctorDriver) -> Result<Self> {
        Ok(DoctorState {
            phase: d.phase.clone(),
            failed: d.failed,
        })
    }
}

impl Driver for DoctorDriver {
    type State = DoctorState;

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => {
                self.phase = "Start".into();
                self.failed = false;
            },
            step => {
                self.phase = "Start".into();
                self.failed = false;
            },
            check_config_ok => {
                if self.phase != "Start" || self.failed {
                    anyhow::bail!("check_config_ok not enabled");
                }
                self.phase = "AfterConfig".into();
            },
            check_config_err => {
                if self.phase != "Start" {
                    anyhow::bail!("check_config_err not enabled");
                }
                self.phase = "Terminal".into();
                self.failed = true;
            },
            check_store_ok => {
                if self.phase != "AfterConfig" || self.failed {
                    anyhow::bail!("check_store_ok not enabled");
                }
                self.phase = "AfterStore".into();
            },
            check_store_err => {
                if self.phase != "AfterConfig" {
                    anyhow::bail!("check_store_err not enabled");
                }
                self.phase = "Terminal".into();
                self.failed = true;
            },
            finish_hooks => {
                if self.phase != "AfterStore" || self.failed {
                    anyhow::bail!("finish_hooks not enabled");
                }
                self.phase = "Done".into();
            },
        })
    }
}

#[quint_run(
    spec = "specs/doctor-diagnostic.qnt",
    max_samples = 12,
    max_steps = 8,
    seed = "0x2"
)]
fn doctor_diagnostic_run() -> impl Driver {
    DoctorDriver::default()
}
