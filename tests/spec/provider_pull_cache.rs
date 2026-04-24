// SPDX-License-Identifier: AGPL-3.0-or-later
// Driver replays pull/upsert/commit; `State = ()` avoids ITF Set/BigInt round-trip in `from_spec`.
use quint_connect::*;
use std::collections::BTreeSet;

#[derive(Debug, Default)]
struct ProviderPullCacheDriver {
    cursor: i64,
    stored: BTreeSet<String>,
    pending: BTreeSet<String>,
    txn_open: bool,
}

impl Driver for ProviderPullCacheDriver {
    type State = ();

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => {
                self.cursor = 0;
                self.stored.clear();
                self.pending.clear();
                self.txn_open = false;
            },
            step => {}
            start_refresh => {
                if !self.txn_open {
                    self.txn_open = true;
                    self.pending.clear();
                }
            },
            upsert(id: String) => {
                if self.txn_open {
                    self.pending.insert(id);
                }
            },
            commit => {
                if self.txn_open {
                    self.stored = std::mem::take(&mut self.pending);
                    self.cursor += 1;
                    self.txn_open = false;
                }
            },
            abort => {
                if self.txn_open {
                    self.pending.clear();
                    self.txn_open = false;
                }
            },
        })
    }
}

#[quint_run(
    spec = "specs/provider-pull-cache.qnt",
    max_samples = 16,
    max_steps = 12
)]
fn provider_pull_cache_run() -> impl Driver {
    ProviderPullCacheDriver::default()
}
