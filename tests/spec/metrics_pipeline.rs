// SPDX-License-Identifier: AGPL-3.0-or-later
use quint_connect::*;
use serde::Deserialize;

#[derive(Debug, Eq, PartialEq, Deserialize)]
struct MetricsPipeState {
    phase: String,
    workspace_kind: String,
    index_succeeded: bool,
    refresh: bool,
    force: bool,
}

#[derive(Debug)]
struct MetricsPipeDriver {
    phase: String,
    workspace_kind: String,
    index_succeeded: bool,
    refresh: bool,
    force: bool,
}

impl Default for MetricsPipeDriver {
    fn default() -> Self {
        Self {
            phase: "Idle".into(),
            workspace_kind: String::new(),
            index_succeeded: false,
            refresh: false,
            force: false,
        }
    }
}

impl State<MetricsPipeDriver> for MetricsPipeState {
    fn from_driver(d: &MetricsPipeDriver) -> Result<Self> {
        Ok(MetricsPipeState {
            phase: d.phase.clone(),
            workspace_kind: d.workspace_kind.clone(),
            index_succeeded: d.index_succeeded,
            refresh: d.refresh,
            force: d.force,
        })
    }
}

impl Driver for MetricsPipeDriver {
    type State = MetricsPipeState;

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => {
                self.phase = "Idle".into();
                self.workspace_kind.clear();
                self.index_succeeded = false;
            },
            step => {
                self.phase = "Idle".into();
                self.workspace_kind.clear();
                self.index_succeeded = false;
            },
            resolve_git_workspace => {
                if self.phase != "Idle" {
                    anyhow::bail!("resolve_git_workspace not enabled");
                }
                self.phase = "Resolved".into();
                self.workspace_kind = "git".into();
                self.index_succeeded = false;
                self.refresh = false;
                self.force = false;
            },
            resolve_plain_workspace => {
                if self.phase != "Idle" {
                    anyhow::bail!("resolve_plain_workspace not enabled");
                }
                self.phase = "Resolved".into();
                self.workspace_kind = "plain".into();
                self.index_succeeded = false;
                self.refresh = false;
                self.force = false;
            },
            resolve_force_index => {
                if self.phase != "Idle" {
                    anyhow::bail!("resolve_force_index not enabled");
                }
                self.phase = "Resolved".into();
                self.workspace_kind = "git".into();
                self.index_succeeded = false;
                self.refresh = false;
                self.force = true;
            },
            resolve_refresh => {
                if self.phase != "Idle" {
                    anyhow::bail!("resolve_refresh not enabled");
                }
                self.phase = "Resolved".into();
                self.workspace_kind = "git".into();
                self.index_succeeded = false;
                self.refresh = true;
                self.force = false;
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
                if !self.refresh {
                    anyhow::bail!("scan_agents requires refresh");
                }
                self.phase = "Scanned".into();
            },
            ensure_index => {
                if self.phase != "StoreOpen" && self.phase != "Scanned" {
                    anyhow::bail!("ensure_index not enabled");
                }
                if !self.force {
                    anyhow::bail!("ensure_index requires force");
                }
                if self.workspace_kind != "git" && self.workspace_kind != "plain" {
                    anyhow::bail!("ensure_index requires workspace kind");
                }
                self.phase = "Indexed".into();
                self.index_succeeded = true;
            },
            build_report => {
                if self.phase != "StoreOpen" && self.phase != "Scanned" && self.phase != "Indexed" {
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
