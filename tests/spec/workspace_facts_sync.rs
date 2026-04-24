// SPDX-License-Identifier: AGPL-3.0-or-later
// Outbox: drain `events` before `workspace_facts` in the model; invariants in spec.
use quint_connect::*;

#[derive(Debug, Default)]
struct WorkspaceFactsSyncDriver {
    n_events: i64,
    n_wf: i64,
    phase: String,
    flush_ok_count: i64,
}

impl Driver for WorkspaceFactsSyncDriver {
    type State = ();

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => {
                self.n_events = 0;
                self.n_wf = 0;
                self.phase = "Done".into();
                self.flush_ok_count = 0;
            }
            init_mixed => {
                self.n_events = 1;
                self.n_wf = 1;
                self.phase = "DrainingEvents".into();
                self.flush_ok_count = 0;
            }
            init_wf_only => {
                self.n_events = 0;
                self.n_wf = 1;
                self.phase = "DrainingWf".into();
                self.flush_ok_count = 0;
            }
            step => {}
            flush_events_batch => {
                if self.phase == "DrainingEvents" && self.n_events > 0 {
                    let ne = self.n_events - 1;
                    self.n_events = ne;
                    self.flush_ok_count += 1;
                    self.phase = if ne == 0 && self.n_wf > 0 {
                        "DrainingWf".into()
                    } else if ne == 0 && self.n_wf == 0 {
                        "Done".into()
                    } else {
                        "DrainingEvents".into()
                    };
                }
            }
            flush_wf_batch => {
                if self.phase == "DrainingWf" && self.n_wf > 0 {
                    let nw = self.n_wf - 1;
                    self.n_wf = nw;
                    self.flush_ok_count += 1;
                    self.phase = if nw == 0 { "Done".into() } else { "DrainingWf".into() };
                }
            }
        })
    }
}

#[quint_run(
    spec = "specs/workspace-facts-sync.qnt",
    max_samples = 20,
    max_steps = 10,
    seed = "0x4"
)]
fn workspace_facts_sync_run() -> impl Driver {
    WorkspaceFactsSyncDriver::default()
}
