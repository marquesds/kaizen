// SPDX-License-Identifier: AGPL-3.0-or-later
use quint_connect::*;
use serde::Deserialize;
use std::fmt::Debug;

#[derive(Debug, Eq, PartialEq, Deserialize)]
struct TelemetryExportersState {
    #[serde(with = "itf::de::As::<itf::de::Integer>")]
    outbox: i64,
    sync_broken: bool,
    secondary_failed: bool,
    fail_open: bool,
    last_primary_ok: bool,
}

#[derive(Default)]
struct TelemetryExportersDriver {
    outbox: i64,
    sync_broken: bool,
    secondary_failed: bool,
    fail_open: bool,
    last_primary_ok: bool,
}

impl State<TelemetryExportersDriver> for TelemetryExportersState {
    fn from_driver(d: &TelemetryExportersDriver) -> Result<Self> {
        Ok(TelemetryExportersState {
            outbox: d.outbox,
            sync_broken: d.sync_broken,
            secondary_failed: d.secondary_failed,
            fail_open: d.fail_open,
            last_primary_ok: d.last_primary_ok,
        })
    }
}

impl Driver for TelemetryExportersDriver {
    type State = TelemetryExportersState;

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => {
                self.outbox = 2;
                self.sync_broken = false;
                self.secondary_failed = false;
                self.fail_open = true;
                self.last_primary_ok = false;
            }
            init_fail_closed => {
                self.outbox = 1;
                self.sync_broken = false;
                self.secondary_failed = false;
                self.fail_open = false;
                self.last_primary_ok = false;
            }
            flush_both_ok => {
                if self.outbox > 0 {
                    self.outbox -= 1;
                }
                self.sync_broken = false;
                self.secondary_failed = false;
                self.last_primary_ok = true;
            }
            flush_primary_ok_secondary_fail_open => {
                if self.outbox > 0 && self.fail_open {
                    self.outbox -= 1;
                }
                self.sync_broken = false;
                self.secondary_failed = true;
                self.last_primary_ok = true;
            }
            flush_primary_ok_secondary_fail_strict => {
                if !self.fail_open {
                    // strict: do not clear
                }
                self.sync_broken = false;
                self.secondary_failed = true;
                self.last_primary_ok = true;
            }
            flush_primary_fail => {
                self.sync_broken = true;
                self.secondary_failed = false;
                self.last_primary_ok = false;
            }
        })
    }
}

#[quint_run(spec = "specs/telemetry-exporters.qnt", max_samples = 12, max_steps = 8)]
fn telemetry_exporters_run() -> impl Driver {
    TelemetryExportersDriver::default()
}
