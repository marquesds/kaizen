// SPDX-License-Identifier: AGPL-3.0-or-later
use quint_connect::*;
use serde::Deserialize;

#[derive(Debug, Eq, PartialEq, Deserialize)]
struct ReadReportPerfState {
    phase: String,
    command: String,
    refresh: bool,
    force: bool,
    provider_stale: bool,
    mutated: bool,
}

#[derive(Debug)]
struct ReadReportPerfDriver {
    phase: String,
    command: String,
    refresh: bool,
    force: bool,
    provider_stale: bool,
    mutated: bool,
}

impl Default for ReadReportPerfDriver {
    fn default() -> Self {
        Self {
            phase: "Idle".into(),
            command: "retro".into(),
            refresh: false,
            force: false,
            provider_stale: false,
            mutated: false,
        }
    }
}

impl State<ReadReportPerfDriver> for ReadReportPerfState {
    fn from_driver(d: &ReadReportPerfDriver) -> Result<Self> {
        Ok(Self {
            phase: d.phase.clone(),
            command: d.command.clone(),
            refresh: d.refresh,
            force: d.force,
            provider_stale: d.provider_stale,
            mutated: d.mutated,
        })
    }
}

impl Driver for ReadReportPerfDriver {
    type State = ReadReportPerfState;

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => *self = Self::default(),
            step => *self = Self::default(),
            choose_default_read => {
                require(&self.phase, "Idle", "choose_default_read")?;
                self.command = "read_report".into();
                self.refresh = false;
                self.force = false;
                self.provider_stale = false;
                self.mutated = false;
                self.phase = "Chosen".into();
            },
            choose_refresh_read => {
                require(&self.phase, "Idle", "choose_refresh_read")?;
                self.command = "read_report".into();
                self.refresh = true;
                self.force = false;
                self.provider_stale = false;
                self.mutated = false;
                self.phase = "Chosen".into();
            },
            choose_force_index => {
                require(&self.phase, "Idle", "choose_force_index")?;
                self.command = "metrics_index".into();
                self.refresh = false;
                self.force = true;
                self.provider_stale = false;
                self.mutated = false;
                self.phase = "Chosen".into();
            },
            choose_stale_provider => {
                require(&self.phase, "Idle", "choose_stale_provider")?;
                self.command = "read_report".into();
                self.refresh = false;
                self.force = false;
                self.provider_stale = true;
                self.mutated = false;
                self.phase = "Chosen".into();
            },
            open_query => {
                require(&self.phase, "Chosen", "open_query")?;
                if self.refresh || self.force || self.provider_stale {
                    anyhow::bail!("query path only for cache-fresh default reads");
                }
                self.phase = "OpenQuery".into();
            },
            load_cached => {
                require(&self.phase, "OpenQuery", "load_cached")?;
                self.phase = "LoadCached".into();
            },
            refresh_cache => {
                require(&self.phase, "Chosen", "refresh_cache")?;
                if !self.refresh && !self.provider_stale {
                    anyhow::bail!("refresh_cache requires refresh or stale provider");
                }
                self.mutated = true;
                self.phase = "RefreshCache".into();
            },
            index_repo => {
                require(&self.phase, "Chosen", "index_repo")?;
                if !self.force {
                    anyhow::bail!("index_repo requires force");
                }
                self.mutated = true;
                self.phase = "IndexRepo".into();
            },
            load_after_mutation => {
                if self.phase != "RefreshCache" && self.phase != "IndexRepo" {
                    anyhow::bail!("load_after_mutation not enabled");
                }
                self.phase = "LoadCached".into();
            },
            render => {
                require(&self.phase, "LoadCached", "render")?;
                self.phase = "Render".into();
            },
            done => {
                require(&self.phase, "Render", "done")?;
                self.phase = "Done".into();
            },
        })
    }
}

fn require(actual: &str, expected: &str, action: &str) -> Result {
    if actual != expected {
        anyhow::bail!("{action} requires {expected}, got {actual}");
    }
    Ok(())
}

#[quint_run(
    spec = "specs/read-report-perf.qnt",
    max_samples = 20,
    max_steps = 10,
    seed = "0x500"
)]
fn read_report_perf_run() -> impl Driver {
    ReadReportPerfDriver::default()
}
