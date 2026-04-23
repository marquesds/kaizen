// SPDX-License-Identifier: AGPL-3.0-or-later
use quint_connect::*;
use serde::Deserialize;
use std::collections::HashSet;

#[derive(Debug, Eq, PartialEq, Deserialize)]
struct BackpressureState {
    #[serde(with = "itf::de::As::<itf::de::Integer>")]
    outbox_len: i64,
    #[serde(with = "itf::de::As::<itf::de::Integer>")]
    batch_max: i64,
    last_post_ok: bool,
}

#[derive(Default)]
struct BackpressureDriver {
    outbox_len: i64,
    batch_max: i64,
    last_post_ok: bool,
    idem_keys_sent: HashSet<String>,
}

impl State<BackpressureDriver> for BackpressureState {
    fn from_driver(d: &BackpressureDriver) -> Result<Self> {
        Ok(BackpressureState {
            outbox_len: d.outbox_len,
            batch_max: d.batch_max,
            last_post_ok: d.last_post_ok,
        })
    }
}

impl Driver for BackpressureDriver {
    type State = BackpressureState;

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => {
                self.outbox_len = 0;
                self.batch_max = 3;
                self.last_post_ok = false;
                self.idem_keys_sent.clear();
            },
            step => {}
            enqueue => {
                self.outbox_len += 1;
            },
            flush(fresh: String) => {
                if self.idem_keys_sent.contains(&fresh) {
                    return Ok(());
                }
                let take = self.outbox_len.min(self.batch_max);
                self.outbox_len -= take;
                self.last_post_ok = true;
                self.idem_keys_sent.insert(fresh);
            }
        })
    }
}

#[quint_run(spec = "specs/sync-backpressure.qnt", max_samples = 12, max_steps = 10)]
fn sync_backpressure_run() -> impl Driver {
    BackpressureDriver::default()
}
