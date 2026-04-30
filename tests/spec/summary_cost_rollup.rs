// SPDX-License-Identifier: AGPL-3.0-or-later
// Invariant: sessions present + zero stored cost ⇒ cost_note flag (summary / MCP JSON).
use quint_connect::*;

#[derive(Debug)]
struct SummaryCostRollupDriver {
    phase: String,
    session_count: i64,
    total_cost_usd_e6: i64,
    cost_note_emitted: bool,
}

impl Default for SummaryCostRollupDriver {
    fn default() -> Self {
        Self {
            phase: "Idle".into(),
            session_count: 0,
            total_cost_usd_e6: 0,
            cost_note_emitted: false,
        }
    }
}

impl Driver for SummaryCostRollupDriver {
    type State = ();

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => {
                self.phase = "Idle".into();
                self.session_count = 0;
                self.total_cost_usd_e6 = 0;
                self.cost_note_emitted = false;
            }
            step => {}
            load_no_sessions => {
                if self.phase == "Idle" {
                    self.phase = "Ready".into();
                    self.session_count = 0;
                    self.total_cost_usd_e6 = 0;
                }
            },
            load_sessions_with_cost => {
                if self.phase == "Idle" {
                    self.phase = "Ready".into();
                    self.session_count = 2;
                    self.total_cost_usd_e6 = 100;
                }
            },
            load_sessions_zero_cost => {
                if self.phase == "Idle" {
                    self.phase = "Ready".into();
                    self.session_count = 3;
                    self.total_cost_usd_e6 = 0;
                }
            },
            emit => {
                if self.phase == "Ready" {
                    self.phase = "Done".into();
                    self.cost_note_emitted =
                        self.session_count > 0 && self.total_cost_usd_e6 == 0;
                }
            },
        })
    }
}

#[quint_run(
    spec = "specs/summary-cost-rollup.qnt",
    max_samples = 20,
    max_steps = 10,
    seed = "0x7"
)]
fn summary_cost_rollup_run() -> impl Driver {
    SummaryCostRollupDriver::default()
}
