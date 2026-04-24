// SPDX-License-Identifier: AGPL-3.0-or-later
// `State = ()`: full state (source, merged set, merge_strokes) uses ITF `#set` / `#bigint`
// shapes that do not round-trip cleanly through the shared `State::from_spec` deserializer;
// the driver still replays every action; merge invariants live in `observe-pipeline.qnt`.
use quint_connect::*;
use std::collections::BTreeSet;

#[derive(Debug)]
struct ObserveDriver {
    phase: String,
    source: String,
    merged: BTreeSet<String>,
    merge_strokes: i64,
}

impl Default for ObserveDriver {
    fn default() -> Self {
        Self {
            phase: "Idle".into(),
            source: "local".into(),
            merged: BTreeSet::new(),
            merge_strokes: 0,
        }
    }
}

impl Driver for ObserveDriver {
    type State = ();

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => {
                self.phase = "Idle".into();
                self.source = "local".into();
                self.merged.clear();
                self.merge_strokes = 0;
            }
            init_mixed => {
                self.phase = "Idle".into();
                self.source = "mixed".into();
                self.merged.clear();
                self.merge_strokes = 0;
            }
            init_provider => {
                self.phase = "Idle".into();
                self.source = "provider".into();
                self.merged.clear();
                self.merge_strokes = 0;
            }
            step => {}
            resolve_workspace => {
                if self.phase == "Idle" {
                    self.phase = "Resolved".into();
                }
            },
            load_config => {
                if self.phase == "Resolved" {
                    self.phase = "ConfigLoaded".into();
                }
            },
            open_store => {
                if self.phase == "ConfigLoaded" {
                    self.phase = "StoreOpen".into();
                }
            },
            scan_agents => {
                if self.phase == "StoreOpen" {
                    self.phase = "Scanned".into();
                }
            },
            contribute(k: String) => {
                if self.phase == "Scanned" {
                    self.merge_strokes += 1;
                    self.merged.insert(k);
                }
            },
            contribute_s1 => {
                if self.phase == "Scanned" {
                    self.merge_strokes += 1;
                    self.merged.insert("s1".into());
                }
            },
            contribute_remote => {
                if self.phase == "Scanned" && (self.source == "provider" || self.source == "mixed") {
                    self.merge_strokes += 1;
                    self.merged.insert("r1".into());
                }
            },
            run_query => {
                if self.phase == "Scanned" {
                    self.phase = "Queried".into();
                }
            },
            emit_output => {
                if self.phase == "Queried" {
                    self.phase = "Done".into();
                }
            },
        })
    }
}

#[quint_run(
    spec = "specs/observe-pipeline.qnt",
    max_samples = 20,
    max_steps = 12,
    seed = "0x4"
)]
fn observe_pipeline_run() -> impl Driver {
    ObserveDriver::default()
}
