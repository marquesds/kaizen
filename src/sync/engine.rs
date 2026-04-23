//! Flush outbox batches: size limits, split on 413, backoff on 429 / transient errors.

use crate::core::config::SyncConfig;
use crate::store::Store;
use crate::sync::client::{PostBatchOutcome, SyncHttpClient};
use crate::sync::outbound::{EventsBatchBody, OutboundEvent};
use anyhow::{Context, Result};
use std::path::Path;
use std::thread;
use std::time::Duration;
use uuid::Uuid;

/// Flush pending outbox rows (all that fit batch constraints per iteration).
pub fn flush_outbox_once(
    store: &Store,
    workspace_root: &Path,
    cfg: &SyncConfig,
    team_salt: &[u8; 32],
) -> Result<FlushStats> {
    if cfg.endpoint.is_empty() {
        return Ok(FlushStats::default());
    }
    let client = SyncHttpClient::new(&cfg.endpoint, &cfg.team_token)?;
    let workspace_hash = crate::sync::outbound::workspace_hash(team_salt, workspace_root);
    let mut stats = FlushStats::default();

    while store.outbox_pending_count()? > 0 {
        let rows = store.list_outbox_pending(10_000)?;
        if rows.is_empty() {
            break;
        }
        let (ids, events) = pack_batch_events(&rows, cfg)?;
        if ids.is_empty() {
            break;
        }
        let body = EventsBatchBody {
            team_id: cfg.team_id.clone(),
            workspace_hash: workspace_hash.clone(),
            events,
        };
        let sent = post_batch_resilient(&client, store, body, &ids)?;
        stats.batches += sent.batches;
        stats.events_sent += sent.events;
    }

    Ok(stats)
}

#[derive(Debug, Default, Clone)]
pub struct FlushStats {
    pub batches: u64,
    pub events_sent: u64,
}

#[derive(Default)]
struct Sent {
    batches: u64,
    events: u64,
}

fn pack_batch_events(
    rows: &[(i64, String)],
    cfg: &SyncConfig,
) -> Result<(Vec<i64>, Vec<OutboundEvent>)> {
    let mut ids = Vec::new();
    let mut events = Vec::new();
    let mut bytes = 0usize;
    let max_ev = cfg.events_per_batch_max.max(1);
    for (id, raw) in rows {
        let ev: OutboundEvent = serde_json::from_str(raw).context("parse outbox payload")?;
        let inc = serde_json::to_vec(&ev)?.len();
        if events.len() >= max_ev {
            break;
        }
        if bytes + inc > cfg.max_body_bytes && !events.is_empty() {
            break;
        }
        bytes += inc;
        ids.push(*id);
        events.push(ev);
    }
    Ok((ids, events))
}

fn post_batch_resilient(
    client: &SyncHttpClient,
    store: &Store,
    body: EventsBatchBody,
    ids: &[i64],
) -> Result<Sent> {
    let mut backoff = Duration::from_millis(200);
    let max_backoff = Duration::from_secs(30);
    let mut server_failures = 0u32;

    loop {
        if body.events.is_empty() {
            return Ok(Sent::default());
        }

        let key = Uuid::now_v7();
        match client.post_events_batch(&body, &key)? {
            PostBatchOutcome::Accepted { .. } | PostBatchOutcome::Conflict => {
                store.mark_outbox_sent(ids)?;
                store.set_sync_state_ok()?;
                return Ok(Sent {
                    batches: 1,
                    events: ids.len() as u64,
                });
            }
            PostBatchOutcome::TooLarge => {
                if body.events.len() <= 1 {
                    store.set_sync_state_error("413: single event too large for server")?;
                    anyhow::bail!(
                        "413: single event too large; tighten redaction or max_body_bytes"
                    );
                }
                let mid = body.events.len() / 2;
                let left_ids = ids[..mid].to_vec();
                let right_ids = ids[mid..].to_vec();
                let left_body = EventsBatchBody {
                    team_id: body.team_id.clone(),
                    workspace_hash: body.workspace_hash.clone(),
                    events: body.events[..mid].to_vec(),
                };
                let right_body = EventsBatchBody {
                    team_id: body.team_id.clone(),
                    workspace_hash: body.workspace_hash.clone(),
                    events: body.events[mid..].to_vec(),
                };
                let a = post_batch_resilient(client, store, left_body, &left_ids)?;
                let b = post_batch_resilient(client, store, right_body, &right_ids)?;
                return Ok(Sent {
                    batches: a.batches + b.batches,
                    events: a.events + b.events,
                });
            }
            PostBatchOutcome::RateLimited(d) => {
                thread::sleep(d);
            }
            PostBatchOutcome::Unauthorized => {
                let msg = "401 unauthorized (check team_token)";
                store.set_sync_state_error(msg)?;
                anyhow::bail!("{msg}");
            }
            PostBatchOutcome::ClientError(c) => {
                let msg = format!("HTTP client error {c}");
                store.set_sync_state_error(&msg)?;
                anyhow::bail!("{msg}");
            }
            PostBatchOutcome::ServerError(c) => {
                server_failures += 1;
                if server_failures > 12 {
                    let msg = format!("HTTP server error {c} (exhausted retries)");
                    store.set_sync_state_error(&msg)?;
                    anyhow::bail!("{msg}");
                }
                thread::sleep(backoff);
                backoff = (backoff * 2).min(max_backoff);
            }
        }
    }
}
