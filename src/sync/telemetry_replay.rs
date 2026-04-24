// SPDX-License-Identifier: AGPL-3.0-or-later
//! Chunk local [`OutboundEvent`] vectors into [`IngestExportBatch`] for telemetry replay (exporter
//! fan-out only). Packing mirrors [`crate::sync::engine`] outbox batch limits.

use crate::core::config::SyncConfig;
use crate::sync::export_batch::IngestExportBatch;
use crate::sync::outbound::{EventsBatchBody, OutboundEvent};
use anyhow::Context;
use anyhow::Result;

/// Split redacted events into `Events` batches using `events_per_batch_max` and `max_body_bytes`.
pub fn chunk_events_into_ingest_batches(
    team_id: String,
    workspace_hash: String,
    events: Vec<OutboundEvent>,
    cfg: &SyncConfig,
) -> Result<Vec<IngestExportBatch>> {
    let max_ev = cfg.events_per_batch_max.max(1);
    let max_bytes = cfg.max_body_bytes;
    let mut batches = Vec::new();
    let mut cur: Vec<OutboundEvent> = Vec::new();
    let mut bytes = 0usize;

    for ev in events {
        let inc = serde_json::to_vec(&ev).context("serialize outbound event for batch sizing")?;
        if !cur.is_empty() && (cur.len() >= max_ev || bytes + inc.len() > max_bytes) {
            batches.push(IngestExportBatch::Events(EventsBatchBody {
                team_id: team_id.clone(),
                workspace_hash: workspace_hash.clone(),
                events: std::mem::take(&mut cur),
            }));
            bytes = 0;
        }
        cur.push(ev);
        bytes += inc.len();
        if cur.len() >= max_ev {
            batches.push(IngestExportBatch::Events(EventsBatchBody {
                team_id: team_id.clone(),
                workspace_hash: workspace_hash.clone(),
                events: std::mem::take(&mut cur),
            }));
            bytes = 0;
        }
    }
    if !cur.is_empty() {
        batches.push(IngestExportBatch::Events(EventsBatchBody {
            team_id,
            workspace_hash,
            events: cur,
        }));
    }
    Ok(batches)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync::outbound::OutboundEvent;
    use serde_json::json;

    fn dummy_ev(payload: serde_json::Value) -> OutboundEvent {
        OutboundEvent {
            session_id_hash: "h".into(),
            event_seq: 0,
            ts_ms: 0,
            agent: "a".into(),
            model: "m".into(),
            kind: "message".into(),
            source: "hook".into(),
            tool: None,
            tool_call_id: None,
            tokens_in: None,
            tokens_out: None,
            reasoning_tokens: None,
            cost_usd_e6: None,
            payload,
        }
    }

    fn cfg_count_only(max: usize) -> SyncConfig {
        SyncConfig {
            events_per_batch_max: max,
            max_body_bytes: 10_000_000,
            ..Default::default()
        }
    }

    #[test]
    fn splits_on_event_count() {
        let cfg = cfg_count_only(2);
        let events: Vec<_> = (0..5)
            .map(|i| {
                let mut e = dummy_ev(json!({"i": i}));
                e.event_seq = i;
                e
            })
            .collect();
        let batches =
            chunk_events_into_ingest_batches("t".into(), "w".into(), events, &cfg).unwrap();
        assert_eq!(batches.len(), 3);
        let counts: Vec<_> = batches
            .iter()
            .map(|b| match b {
                IngestExportBatch::Events(e) => e.events.len(),
                _ => panic!("expected events batch"),
            })
            .collect();
        assert_eq!(counts, vec![2, 2, 1]);
    }

    #[test]
    fn splits_on_byte_budget() {
        let cfg = SyncConfig {
            events_per_batch_max: 100,
            max_body_bytes: 50,
            ..Default::default()
        };
        let events = vec![
            dummy_ev(json!({"x": "aa"})),
            dummy_ev(json!({"x": "bbbbbbbbbb"})),
            dummy_ev(json!({"x": "cc"})),
        ];
        let batches =
            chunk_events_into_ingest_batches("t".into(), "w".into(), events, &cfg).unwrap();
        assert!(batches.len() >= 2);
        for b in &batches {
            let IngestExportBatch::Events(body) = b else {
                panic!();
            };
            let ser = serde_json::to_vec(&body.events).unwrap();
            assert!(ser.len() <= cfg.max_body_bytes || body.events.len() == 1);
        }
    }
}
