// SPDX-License-Identifier: AGPL-3.0-or-later
use quint_connect::*;
use serde::Deserialize;

#[derive(Debug, Eq, PartialEq, Deserialize)]
struct TuiState {
    phase: String,
    dirty: bool,
    #[serde(rename = "reportDirty")]
    report_dirty: bool,
    #[serde(rename = "reportComputing")]
    report_computing: bool,
    #[serde(rename = "reportReady")]
    report_ready: bool,
    #[serde(rename = "walEvents")]
    wal_events: i64,
    #[serde(rename = "manualRefreshes")]
    manual_refreshes: i64,
    refreshes: i64,
    #[serde(rename = "reportPublishes")]
    report_publishes: i64,
}

#[derive(Debug)]
struct TuiDriver {
    phase: String,
    dirty: bool,
    report_dirty: bool,
    report_computing: bool,
    report_ready: bool,
    wal_events: i64,
    manual_refreshes: i64,
    refreshes: i64,
    report_publishes: i64,
}

impl Default for TuiDriver {
    fn default() -> Self {
        Self {
            phase: "Boot".into(),
            dirty: false,
            report_dirty: true,
            report_computing: false,
            report_ready: false,
            wal_events: 0,
            manual_refreshes: 0,
            refreshes: 0,
            report_publishes: 0,
        }
    }
}

impl State<TuiDriver> for TuiState {
    fn from_driver(d: &TuiDriver) -> Result<Self> {
        Ok(TuiState {
            phase: d.phase.clone(),
            dirty: d.dirty,
            report_dirty: d.report_dirty,
            report_computing: d.report_computing,
            report_ready: d.report_ready,
            wal_events: d.wal_events,
            manual_refreshes: d.manual_refreshes,
            refreshes: d.refreshes,
            report_publishes: d.report_publishes,
        })
    }
}

impl Driver for TuiDriver {
    type State = TuiState;

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => {
                *self = Self::default();
            },
            step => {
                *self = Self::default();
            },
            become_ready => {
                require_phase(&self.phase, "Boot", "become_ready")?;
                self.phase = "Interactive".into();
            },
            wal_change => {
                require_phase(&self.phase, "Interactive", "wal_change")?;
                self.dirty = true;
                self.wal_events += 1;
            },
            coalesced_refresh => {
                require_phase(&self.phase, "Interactive", "coalesced_refresh")?;
                if !self.dirty {
                    anyhow::bail!("coalesced_refresh not enabled");
                }
                self.dirty = false;
                self.report_dirty = true;
                self.refreshes += 1;
            },
            manual_refresh => {
                require_phase(&self.phase, "Interactive", "manual_refresh")?;
                self.dirty = false;
                self.report_dirty = true;
                self.manual_refreshes += 1;
                self.refreshes += 1;
            },
            report_recompute => {
                require_phase(&self.phase, "Interactive", "report_recompute")?;
                if !self.report_dirty || self.report_computing {
                    anyhow::bail!("report_recompute not enabled");
                }
                self.report_dirty = false;
                self.report_computing = true;
            },
            report_publish => {
                require_phase(&self.phase, "Interactive", "report_publish")?;
                if !self.report_computing {
                    anyhow::bail!("report_publish not enabled");
                }
                self.report_computing = false;
                self.report_ready = true;
                self.report_publishes += 1;
            },
            request_quit => {
                require_phase(&self.phase, "Interactive", "request_quit")?;
                self.phase = "Draining".into();
            },
            finish_shutdown => {
                require_phase(&self.phase, "Draining", "finish_shutdown")?;
                self.phase = "Exited".into();
            },
        })
    }
}

fn require_phase(actual: &str, expected: &str, action: &str) -> Result<()> {
    if actual != expected {
        anyhow::bail!("{action} not enabled");
    }
    Ok(())
}

#[quint_run(
    spec = "specs/tui-app.qnt",
    max_samples = 20,
    max_steps = 10,
    seed = "0x7"
)]
fn tui_app_run() -> impl Driver {
    TuiDriver::default()
}
