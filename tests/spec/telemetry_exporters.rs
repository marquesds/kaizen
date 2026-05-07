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
    #[serde(with = "itf::de::As::<itf::de::Integer>")]
    dd_records_emitted: i64,
    dd_emit_has_timestamp: bool,
    dd_emit_has_hostname: bool,
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
    dd_records_emitted: i64,
    dd_emit_has_timestamp: bool,
    dd_emit_has_hostname: bool,
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
            dd_records_emitted: d.dd_records_emitted,
            dd_emit_has_timestamp: d.dd_emit_has_timestamp,
            dd_emit_has_hostname: d.dd_emit_has_hostname,
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
                self.dd_records_emitted = 0;
                self.dd_emit_has_timestamp = true;
                self.dd_emit_has_hostname = true;
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
                self.dd_records_emitted = 0;
                self.dd_emit_has_timestamp = true;
                self.dd_emit_has_hostname = true;
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
            emit_dd_record => {
                let (has_ts, has_host) = build_real_dd_record();
                self.dd_records_emitted += 1;
                self.dd_emit_has_timestamp = has_ts;
                self.dd_emit_has_hostname = has_host;
            }
            emit_dd_span_record => {
                let (has_ts, has_host) = build_real_dd_span_record();
                self.dd_records_emitted += 1;
                self.dd_emit_has_timestamp = has_ts;
                self.dd_emit_has_hostname = has_host;
            }
            step => {}
        })
    }
}

/// Build one DD log object via the real `kaizen::telemetry::datadog::dd_log_object` and
/// inspect the resulting JSON. Driver-side observation is what gives the spec invariant
/// `dd_records_well_formed` real teeth: a regression that drops `timestamp` or `hostname`
/// flips one bool to false and the spec rejects the trace.
#[cfg(feature = "telemetry-datadog")]
fn build_real_dd_record() -> (bool, bool) {
    use kaizen::sync::IngestExportBatch;
    use kaizen::sync::canonical::expand_ingest_batch;
    use kaizen::sync::outbound::{EventsBatchBody, OutboundEvent};
    let b = IngestExportBatch::Events(EventsBatchBody {
        team_id: "t".into(),
        workspace_hash: "wh".into(),
        events: vec![OutboundEvent {
            session_id_hash: "sid".into(),
            event_seq: 0,
            ts_ms: 1,
            agent: "kaizen".into(),
            model: "synthetic".into(),
            kind: "lifecycle".into(),
            source: "tail".into(),
            tool: None,
            tool_call_id: None,
            tokens_in: None,
            tokens_out: None,
            reasoning_tokens: None,
            cost_usd_e6: None,
            payload: serde_json::json!({}),
        }],
    });
    let items = expand_ingest_batch(&b);
    let v = kaizen::telemetry::datadog::dd_log_object_for_test(&items[0], "spec-host");
    (v.get("timestamp").is_some(), v.get("hostname").is_some())
}

#[cfg(not(feature = "telemetry-datadog"))]
fn build_real_dd_record() -> (bool, bool) {
    (true, true)
}

/// Same idea as `build_real_dd_record`, but exercises the [`OutboundToolSpan`] branch of
/// `dd_log_object`. `kaizen telemetry push` emits both kinds, so the spec invariant
/// `dd_records_well_formed` only has teeth on the span path if the driver actually constructs
/// one through the real builder.
#[cfg(feature = "telemetry-datadog")]
fn build_real_dd_span_record() -> (bool, bool) {
    use kaizen::sync::IngestExportBatch;
    use kaizen::sync::canonical::expand_ingest_batch;
    use kaizen::sync::smart::{OutboundToolSpan, ToolSpansBatchBody};
    let b = IngestExportBatch::ToolSpans(ToolSpansBatchBody {
        team_id: "t".into(),
        workspace_hash: "wh".into(),
        spans: vec![OutboundToolSpan {
            session_id_hash: "sid".into(),
            span_id_hash: "span".into(),
            tool: Some("Read".into()),
            status: "ok".into(),
            started_at_ms: Some(1),
            ended_at_ms: Some(2),
            lead_time_ms: Some(1),
            tokens_in: None,
            tokens_out: None,
            reasoning_tokens: None,
            cost_usd_e6: None,
            path_hashes: vec!["blake3:p".into()],
        }],
    });
    let items = expand_ingest_batch(&b);
    let v = kaizen::telemetry::datadog::dd_log_object_for_test(&items[0], "spec-host");
    (v.get("timestamp").is_some(), v.get("hostname").is_some())
}

#[cfg(not(feature = "telemetry-datadog"))]
fn build_real_dd_span_record() -> (bool, bool) {
    (true, true)
}

#[quint_run(
    spec = "specs/telemetry-exporters.qnt",
    max_samples = 20,
    max_steps = 14
)]
fn telemetry_exporters_run() -> impl Driver {
    TelemetryExportersDriver::default()
}
