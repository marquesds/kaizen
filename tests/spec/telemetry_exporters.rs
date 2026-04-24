// SPDX-License-Identifier: AGPL-3.0-or-later
use itf::value::Value;
use quint_connect::*;
use serde::Deserialize;

#[derive(Debug, Eq, PartialEq, Deserialize)]
struct TelemetryExportersState {
    #[serde(with = "itf::de::As::<itf::de::Integer>")]
    outbox: i64,
    sync_broken: bool,
    secondary_failed: bool,
    fail_open: bool,
    last_primary_ok: bool,
    #[serde(with = "itf::de::As::<itf::de::Integer>")]
    fanout_n: i64,
    query_authority: String,
    import_window_open: bool,
    import_from: String,
}

#[derive(Default)]
struct TelemetryExportersDriver {
    outbox: i64,
    sync_broken: bool,
    secondary_failed: bool,
    fail_open: bool,
    last_primary_ok: bool,
    fanout_n: i64,
    query_authority: String,
    import_window_open: bool,
    import_from: String,
}

impl State<TelemetryExportersDriver> for TelemetryExportersState {
    fn from_driver(d: &TelemetryExportersDriver) -> Result<Self> {
        Ok(TelemetryExportersState {
            outbox: d.outbox,
            sync_broken: d.sync_broken,
            secondary_failed: d.secondary_failed,
            fail_open: d.fail_open,
            last_primary_ok: d.last_primary_ok,
            fanout_n: d.fanout_n,
            query_authority: d.query_authority.clone(),
            import_window_open: d.import_window_open,
            import_from: d.import_from.clone(),
        })
    }
}

impl TelemetryExportersDriver {
    fn read_authority_step(step: &Step) -> Result<String> {
        let v = step
            .nondet_picks
            .get("a")
            .ok_or_else(|| anyhow::anyhow!("nondet pick `a` for set_query_authority"))?;
        match v {
            Value::String(s) => Ok(s.clone()),
            _ => anyhow::bail!("nondet `a` was not a string: {v:?}"),
        }
    }

    fn read_fanout_n(step: &Step) -> Result<i64> {
        let v = step
            .nondet_picks
            .get("n")
            .ok_or_else(|| anyhow::anyhow!("nondet pick `n` for set_fanout"))?;
        match v {
            Value::Number(n) => Ok(*n),
            Value::BigInt(n) => format!("{n}")
                .parse::<i64>()
                .map_err(|e| anyhow::anyhow!("nondet `n` bigint: {e}")),
            _ => anyhow::bail!("nondet `n` was not a number: {v:?}"),
        }
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
                self.fanout_n = 2;
                self.query_authority = "none".into();
                self.import_window_open = false;
                self.import_from = "none".into();
            }
            init_fail_closed => {
                self.outbox = 1;
                self.sync_broken = false;
                self.secondary_failed = false;
                self.fail_open = false;
                self.last_primary_ok = false;
                self.fanout_n = 2;
                self.query_authority = "none".into();
                self.import_window_open = false;
                self.import_from = "none".into();
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
            set_query_authority => {
                if self.import_window_open {
                    return Ok(());
                }
                self.query_authority = Self::read_authority_step(step)?;
                self.import_window_open = false;
                self.import_from = "none".into();
            }
            set_fanout => {
                if self.import_window_open {
                    return Ok(());
                }
                self.fanout_n = Self::read_fanout_n(step)?;
                self.import_window_open = false;
                self.import_from = "none".into();
            }
            import_begin => {
                if !self.import_window_open && self.query_authority != "none" {
                    self.import_window_open = true;
                    self.import_from = self.query_authority.clone();
                }
            }
            import_end => {
                if self.import_window_open {
                    self.import_window_open = false;
                    self.import_from = "none".into();
                }
            }
            step => {}
        })
    }
}

#[quint_run(
    spec = "specs/telemetry-exporters.qnt",
    max_samples = 20,
    max_steps = 14
)]
fn telemetry_exporters_run() -> impl Driver {
    TelemetryExportersDriver::default()
}
