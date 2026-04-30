// SPDX-License-Identifier: AGPL-3.0-or-later
use quint_connect::*;
use serde::Deserialize;

#[derive(Debug, Eq, PartialEq, Deserialize)]
struct RetroPipelineState {
    phase: String,
    #[serde(rename = "lockHeld")]
    lock_held: bool,
    source: String,
    #[serde(rename = "cache_fresh")]
    cache_fresh: bool,
    #[serde(rename = "wants_refresh")]
    wants_refresh: bool,
    #[serde(rename = "indexed_repo")]
    indexed_repo: bool,
}

#[derive(Debug, Default)]
struct RetroDriver {
    phase: String,
    lock_held: bool,
    source: String,
    cache_fresh: bool,
    wants_refresh: bool,
    indexed_repo: bool,
}

impl State<RetroDriver> for RetroPipelineState {
    fn from_driver(d: &RetroDriver) -> Result<Self> {
        Ok(RetroPipelineState {
            phase: d.phase.clone(),
            lock_held: d.lock_held,
            source: d.source.clone(),
            cache_fresh: d.cache_fresh,
            wants_refresh: d.wants_refresh,
            indexed_repo: d.indexed_repo,
        })
    }
}

impl Driver for RetroDriver {
    type State = RetroPipelineState;

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => {
                self.phase = "Idle".into();
                self.lock_held = false;
                self.source = "local".into();
                self.cache_fresh = true;
                self.wants_refresh = false;
                self.indexed_repo = false;
            },
            init_with_remote_source => {
                self.phase = "Idle".into();
                self.lock_held = false;
                self.source = "provider".into();
                self.cache_fresh = true;
                self.wants_refresh = false;
                self.indexed_repo = false;
            },
            init_stale_provider => {
                self.phase = "Idle".into();
                self.lock_held = false;
                self.source = "provider".into();
                self.cache_fresh = false;
                self.wants_refresh = false;
                self.indexed_repo = false;
            },
            init_refresh_local => {
                self.phase = "Idle".into();
                self.lock_held = false;
                self.source = "local".into();
                self.cache_fresh = true;
                self.wants_refresh = true;
                self.indexed_repo = false;
            },
            step => {}
            acquire => {
                if self.phase == "Idle" && !self.lock_held && self.source == "local" {
                    self.lock_held = true;
                    self.phase = "Loading".into();
                }
            },
            index_repo => {
                if self.phase != "Loading" || !self.wants_refresh {
                    anyhow::bail!("index_repo not enabled");
                }
                self.indexed_repo = true;
            },
            acquire_from_cache => {
                if self.phase == "Idle" && !self.lock_held
                    && (self.source == "provider" || self.source == "mixed")
                    && self.cache_fresh
                    && !self.wants_refresh
                {
                    self.lock_held = true;
                    self.phase = "Loading".into();
                }
            },
            acquire_start_pull => {
                if self.phase == "Idle" && !self.lock_held
                    && (self.source == "provider" || self.source == "mixed")
                    && (!self.cache_fresh || self.wants_refresh)
                {
                    self.lock_held = true;
                    self.phase = "RemotePull".into();
                }
            },
            pull_done => {
                if self.phase == "RemotePull" {
                    self.phase = "Loading".into();
                    self.cache_fresh = true;
                    self.wants_refresh = false;
                }
            },
            load_done => {
                if self.phase == "Loading" {
                    self.phase = "Computing".into();
                }
            },
            compute_done => {
                if self.phase == "Computing" {
                    self.phase = "Ranking".into();
                }
            },
            rank_done => {
                if self.phase == "Ranking" {
                    self.phase = "Writing".into();
                }
            },
            write_done => {
                if self.phase == "Writing" {
                    self.lock_held = false;
                    self.phase = "Idle".into();
                }
            },
        })
    }
}

#[quint_run(spec = "specs/retro-pipeline.qnt", max_samples = 20, max_steps = 12)]
fn retro_pipeline_run() -> impl Driver {
    RetroDriver::default()
}
