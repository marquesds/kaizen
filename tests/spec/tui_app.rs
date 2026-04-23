// SPDX-License-Identifier: AGPL-3.0-or-later
use quint_connect::*;
use serde::Deserialize;

#[derive(Debug, Eq, PartialEq, Deserialize)]
struct TuiState {
    phase: String,
}

#[derive(Debug)]
struct TuiDriver {
    phase: String,
}

impl Default for TuiDriver {
    fn default() -> Self {
        Self {
            phase: "Boot".into(),
        }
    }
}

impl State<TuiDriver> for TuiState {
    fn from_driver(d: &TuiDriver) -> Result<Self> {
        Ok(TuiState {
            phase: d.phase.clone(),
        })
    }
}

impl Driver for TuiDriver {
    type State = TuiState;

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => {
                self.phase = "Boot".into();
            },
            step => {
                self.phase = "Boot".into();
            },
            become_ready => {
                if self.phase != "Boot" {
                    anyhow::bail!("become_ready not enabled");
                }
                self.phase = "Interactive".into();
            },
            request_quit => {
                if self.phase != "Interactive" {
                    anyhow::bail!("request_quit not enabled");
                }
                self.phase = "Draining".into();
            },
            finish_shutdown => {
                if self.phase != "Draining" {
                    anyhow::bail!("finish_shutdown not enabled");
                }
                self.phase = "Exited".into();
            },
        })
    }
}

#[quint_run(
    spec = "specs/tui-app.qnt",
    max_samples = 10,
    max_steps = 8,
    seed = "0x7"
)]
fn tui_app_run() -> impl Driver {
    TuiDriver::default()
}
