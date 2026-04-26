// SPDX-License-Identifier: AGPL-3.0-or-later
use itf::value::Value;
use quint_connect::*;
use serde::Deserialize;

#[derive(Debug, Eq, PartialEq, Deserialize)]
struct TfmState {
    #[serde(with = "itf::de::As::<itf::de::Integer>")]
    ndjson_lines: i64,
    file_sink_enabled: bool,
    #[serde(with = "itf::de::As::<itf::de::Integer>")]
    tail_delivered: i64,
    last_includes_envelope: bool,
    last_batch_kind: String,
}

#[derive(Default)]
struct TfmDriver {
    ndjson_lines: i64,
    file_sink_enabled: bool,
    tail_delivered: i64,
    last_includes_envelope: bool,
    last_batch_kind: String,
}

impl State<TfmDriver> for TfmState {
    fn from_driver(d: &TfmDriver) -> Result<Self> {
        Ok(TfmState {
            ndjson_lines: d.ndjson_lines,
            file_sink_enabled: d.file_sink_enabled,
            tail_delivered: d.tail_delivered,
            last_includes_envelope: d.last_includes_envelope,
            last_batch_kind: d.last_batch_kind.clone(),
        })
    }
}

impl TfmDriver {
    fn read_k(step: &Step) -> Result<String> {
        let v = step
            .nondet_picks
            .get("k")
            .ok_or_else(|| anyhow::anyhow!("nondet pick `k`"))?;
        match v {
            Value::String(s) => Ok(s.clone()),
            _ => anyhow::bail!("nondet `k` not string: {v:?}"),
        }
    }
}

impl Driver for TfmDriver {
    type State = TfmState;

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => {
                self.ndjson_lines = 0;
                self.file_sink_enabled = false;
                self.tail_delivered = 0;
                self.last_includes_envelope = false;
                self.last_batch_kind.clear();
            }
            enable_file_sink => {
                self.file_sink_enabled = true;
            }
            export_enveloped => {
                if self.file_sink_enabled {
                    self.ndjson_lines += 1;
                    self.last_includes_envelope = true;
                    self.last_batch_kind = Self::read_k(step)?;
                }
            }
            export_body_only => {
                if self.file_sink_enabled {
                    self.ndjson_lines += 1;
                    self.last_includes_envelope = false;
                    self.last_batch_kind = Self::read_k(step)?;
                }
            }
            export_file_disabled => {}
            tail_catchup => {
                self.tail_delivered = self.ndjson_lines;
            }
        })
    }
}

#[quint_run(
    spec = "specs/telemetry-file-metadata.qnt",
    max_samples = 25,
    max_steps = 16
)]
fn telemetry_file_metadata_run() -> impl Driver {
    TfmDriver::default()
}
