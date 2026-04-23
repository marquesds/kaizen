//! Flush outbox batches: size limits, split on 413, backoff on 429 / transient errors.

use crate::core::config::SyncConfig;
use crate::store::Store;
use crate::sync::client::{PostBatchOutcome, SyncHttpClient};
use crate::sync::outbound::{EventsBatchBody, OutboundEvent};
use crate::sync::smart::{
    OutboundRepoSnapshotChunk, OutboundToolSpan, RepoSnapshotsBatchBody, ToolSpansBatchBody,
};
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
        let Some(kind) = rows.first().map(|(_, kind, _)| kind.clone()) else {
            break;
        };
        let sent = match build_batch(&rows, cfg, &cfg.team_id, &workspace_hash, &kind)? {
            Some((ids, batch)) => post_batch_resilient(&client, store, batch, &ids)?,
            None => break,
        };
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

fn build_batch(
    rows: &[(i64, String, String)],
    cfg: &SyncConfig,
    team_id: &str,
    workspace_hash: &str,
    kind: &str,
) -> Result<Option<(Vec<i64>, PendingBatch)>> {
    match kind {
        "events" => {
            let (ids, events) = pack_batch_payloads::<OutboundEvent>(rows, cfg, kind)?;
            if ids.is_empty() {
                return Ok(None);
            }
            Ok(Some((
                ids,
                PendingBatch::Events(EventsBatchBody {
                    team_id: team_id.into(),
                    workspace_hash: workspace_hash.into(),
                    events,
                }),
            )))
        }
        "tool_spans" => {
            let (ids, spans) = pack_batch_payloads::<OutboundToolSpan>(rows, cfg, kind)?;
            if ids.is_empty() {
                return Ok(None);
            }
            Ok(Some((
                ids,
                PendingBatch::ToolSpans(ToolSpansBatchBody {
                    team_id: team_id.into(),
                    workspace_hash: workspace_hash.into(),
                    spans,
                }),
            )))
        }
        "repo_snapshots" => {
            let (ids, snapshots) =
                pack_batch_payloads::<OutboundRepoSnapshotChunk>(rows, cfg, kind)?;
            if ids.is_empty() {
                return Ok(None);
            }
            Ok(Some((
                ids,
                PendingBatch::RepoSnapshots(RepoSnapshotsBatchBody {
                    team_id: team_id.into(),
                    workspace_hash: workspace_hash.into(),
                    snapshots,
                }),
            )))
        }
        _ => Ok(None),
    }
}

fn pack_batch_payloads<T>(
    rows: &[(i64, String, String)],
    cfg: &SyncConfig,
    kind: &str,
) -> Result<(Vec<i64>, Vec<T>)>
where
    T: serde::de::DeserializeOwned + serde::Serialize,
{
    let mut ids = Vec::new();
    let mut out = Vec::new();
    let mut bytes = 0usize;
    let max_ev = cfg.events_per_batch_max.max(1);
    for (id, row_kind, raw) in rows {
        if row_kind != kind {
            break;
        }
        let item: T = serde_json::from_str(raw).context("parse outbox payload")?;
        let inc = serde_json::to_vec(&item)?.len();
        if out.len() >= max_ev {
            break;
        }
        if bytes + inc > cfg.max_body_bytes && !out.is_empty() {
            break;
        }
        bytes += inc;
        ids.push(*id);
        out.push(item);
    }
    Ok((ids, out))
}

enum PendingBatch {
    Events(EventsBatchBody),
    ToolSpans(ToolSpansBatchBody),
    RepoSnapshots(RepoSnapshotsBatchBody),
}

fn post_batch_resilient(
    client: &SyncHttpClient,
    store: &Store,
    body: PendingBatch,
    ids: &[i64],
) -> Result<Sent> {
    let mut backoff = Duration::from_millis(200);
    let max_backoff = Duration::from_secs(30);
    let mut server_failures = 0u32;

    loop {
        if batch_len(&body) == 0 {
            return Ok(Sent::default());
        }

        let key = Uuid::now_v7();
        let outcome = match &body {
            PendingBatch::Events(body) => client.post_events_batch(body, &key)?,
            PendingBatch::ToolSpans(body) => client.post_tool_spans_batch(body, &key)?,
            PendingBatch::RepoSnapshots(body) => client.post_repo_snapshots_batch(body, &key)?,
        };
        match outcome {
            PostBatchOutcome::Accepted { .. } | PostBatchOutcome::Conflict => {
                store.mark_outbox_sent(ids)?;
                store.set_sync_state_ok()?;
                return Ok(Sent {
                    batches: 1,
                    events: ids.len() as u64,
                });
            }
            PostBatchOutcome::TooLarge => {
                if batch_len(&body) <= 1 {
                    store.set_sync_state_error("413: single event too large for server")?;
                    anyhow::bail!(
                        "413: single event too large; tighten redaction or max_body_bytes"
                    );
                }
                let mid = batch_len(&body) / 2;
                let left_ids = ids[..mid].to_vec();
                let right_ids = ids[mid..].to_vec();
                let (left_body, right_body) = split_batch(body, mid);
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

fn batch_len(body: &PendingBatch) -> usize {
    match body {
        PendingBatch::Events(body) => body.events.len(),
        PendingBatch::ToolSpans(body) => body.spans.len(),
        PendingBatch::RepoSnapshots(body) => body.snapshots.len(),
    }
}

fn split_batch(body: PendingBatch, mid: usize) -> (PendingBatch, PendingBatch) {
    match body {
        PendingBatch::Events(body) => (
            PendingBatch::Events(EventsBatchBody {
                team_id: body.team_id.clone(),
                workspace_hash: body.workspace_hash.clone(),
                events: body.events[..mid].to_vec(),
            }),
            PendingBatch::Events(EventsBatchBody {
                team_id: body.team_id,
                workspace_hash: body.workspace_hash,
                events: body.events[mid..].to_vec(),
            }),
        ),
        PendingBatch::ToolSpans(body) => (
            PendingBatch::ToolSpans(ToolSpansBatchBody {
                team_id: body.team_id.clone(),
                workspace_hash: body.workspace_hash.clone(),
                spans: body.spans[..mid].to_vec(),
            }),
            PendingBatch::ToolSpans(ToolSpansBatchBody {
                team_id: body.team_id,
                workspace_hash: body.workspace_hash,
                spans: body.spans[mid..].to_vec(),
            }),
        ),
        PendingBatch::RepoSnapshots(body) => (
            PendingBatch::RepoSnapshots(RepoSnapshotsBatchBody {
                team_id: body.team_id.clone(),
                workspace_hash: body.workspace_hash.clone(),
                snapshots: body.snapshots[..mid].to_vec(),
            }),
            PendingBatch::RepoSnapshots(RepoSnapshotsBatchBody {
                team_id: body.team_id,
                workspace_hash: body.workspace_hash,
                snapshots: body.snapshots[mid..].to_vec(),
            }),
        ),
    }
}
