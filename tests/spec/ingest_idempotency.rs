// SPDX-License-Identifier: AGPL-3.0-or-later
use quint_connect::*;
use serde::Deserialize;
use std::collections::HashSet;

// --- State (mirrors Quint vars tracked for verification) ---

#[derive(Debug, Eq, PartialEq, Deserialize)]
struct IdempotencyState {
    #[serde(with = "itf::de::As::<itf::de::Integer>")]
    last_status: i64,
    last_was_dup: bool,
}

// --- Driver ---

#[derive(Default)]
struct IdempotencyDriver {
    seen_keys: HashSet<String>,
    last_status: i64,
    last_was_dup: bool,
}

impl State<IdempotencyDriver> for IdempotencyState {
    fn from_driver(d: &IdempotencyDriver) -> Result<Self> {
        Ok(IdempotencyState {
            last_status: d.last_status,
            last_was_dup: d.last_was_dup,
        })
    }
}

impl Driver for IdempotencyDriver {
    type State = IdempotencyState;

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => {
                self.seen_keys.clear();
                self.last_status = 0;
                self.last_was_dup = false;
            },
            step => {
                self.seen_keys.clear();
                self.last_status = 0;
                self.last_was_dup = false;
            },
            receive_new(key: String) => {
                self.seen_keys.insert(key);
                self.last_status = 202;
                self.last_was_dup = false;
            },
            receive_dup(key: String) => {
                let _ = key; // key is in seen_keys; only status changes
                self.last_status = 409;
                self.last_was_dup = true;
            }
        })
    }
}

// --- Test ---

#[quint_run(spec = "specs/ingest-idempotency.qnt", max_samples = 10, max_steps = 5)]
fn ingest_idempotency_run() -> impl Driver {
    IdempotencyDriver::default()
}
