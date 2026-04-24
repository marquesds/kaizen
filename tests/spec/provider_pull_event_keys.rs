// SPDX-License-Identifier: AGPL-3.0-or-later
// Composite event keys `h#seq`; duplicate upsert in one window → one row.
use quint_connect::*;
use std::collections::BTreeSet;

#[derive(Debug, Default)]
struct ProviderPullEventKeysDriver {
    cursor: i64,
    stored: BTreeSet<String>,
    pending: BTreeSet<String>,
    txn_open: bool,
}

impl Driver for ProviderPullEventKeysDriver {
    type State = ();

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => {
                self.cursor = 0;
                self.stored.clear();
                self.pending.clear();
                self.txn_open = false;
            }
            step => {}
            start_refresh => {
                if !self.txn_open {
                    self.txn_open = true;
                    self.pending.clear();
                }
            }
            upsert_event_key(k: String) => {
                if self.txn_open {
                    self.pending.insert(k);
                }
            }
            upsert_sh0 => {
                if self.txn_open {
                    self.pending.insert("sh#0".into());
                }
            }
            commit => {
                if self.txn_open {
                    self.stored = std::mem::take(&mut self.pending);
                    self.cursor += 1;
                    self.txn_open = false;
                }
            }
        })
    }
}

#[quint_run(
    spec = "specs/provider-pull-event-keys.qnt",
    max_samples = 16,
    max_steps = 14,
    seed = "0x4"
)]
fn provider_pull_event_keys_run() -> impl Driver {
    ProviderPullEventKeysDriver::default()
}
