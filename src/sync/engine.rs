// SPDX-License-Identifier: AGPL-3.0-or-later
//! Flush outbox batches: size limits, split on 413, backoff on 429 / transient errors.
//! Optional `FlushExporters` runs HTTP fan-out in parallel with the primary Kaizen POST; only
//! a successful (or 409) primary result commits outbox. Secondary `Err` is observed only in that
//! same step (and blocks commit when `fail_open` is `false`).

use crate::core::config::TelemetryConfig;
use crate::store::Store;
use crate::sync::IngestExportBatch;
use crate::sync::client::{PostBatchOutcome, SyncHttpClient};
use crate::sync::outbound::{EventsBatchBody, OutboundEvent};
use crate::sync::smart::{
    OutboundRepoSnapshotChunk, OutboundToolSpan, OutboundWorkspaceFactRow, RepoSnapshotsBatchBody,
    ToolSpansBatchBody, WorkspaceFactsBatchBody,
};
use crate::telemetry::ExporterRegistry;
use anyhow::Context;
use anyhow::Result;
use std::path::Path;
use std::thread;
use std::time::Duration;
use uuid::Uuid;

/// Context for optional pluggable sinks (see [`crate::telemetry`]). Only holds references; copy freely.
#[derive(Clone, Copy)]
pub struct FlushExporters<'a> {
    pub telemetry: &'a TelemetryConfig,
    pub registry: Option<&'a ExporterRegistry>,
}

/// Flush pending outbox rows (all that fit batch constraints per iteration).
pub fn flush_outbox_once(
    store: &Store,
    workspace_root: &Path,
    cfg: &crate::core::config::SyncConfig,
    team_salt: &[u8; 32],
    flush: &FlushExporters<'_>,
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
        let Some(kind) = rows.first().map(|(_, k, _)| k.clone()) else {
            break;
        };
        let sent = match build_batch(&rows, cfg, &cfg.team_id, &workspace_hash, &kind)? {
            Some((ids, batch)) => post_batch_resilient(&client, store, batch, &ids, flush)?,
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
    cfg: &crate::core::config::SyncConfig,
    team_id: &str,
    workspace_hash: &str,
    kind: &str,
) -> Result<Option<(Vec<i64>, IngestExportBatch)>> {
    match kind {
        "events" => {
            let (ids, events) = pack_batch_payloads::<OutboundEvent>(rows, cfg, kind)?;
            if ids.is_empty() {
                return Ok(None);
            }
            Ok(Some((
                ids,
                IngestExportBatch::Events(EventsBatchBody {
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
                IngestExportBatch::ToolSpans(ToolSpansBatchBody {
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
                IngestExportBatch::RepoSnapshots(RepoSnapshotsBatchBody {
                    team_id: team_id.into(),
                    workspace_hash: workspace_hash.into(),
                    snapshots,
                }),
            )))
        }
        "workspace_facts" => {
            let (ids, facts) = pack_batch_payloads::<OutboundWorkspaceFactRow>(rows, cfg, kind)?;
            if ids.is_empty() {
                return Ok(None);
            }
            Ok(Some((
                ids,
                IngestExportBatch::WorkspaceFacts(WorkspaceFactsBatchBody {
                    team_id: team_id.into(),
                    workspace_hash: workspace_hash.into(),
                    facts,
                }),
            )))
        }
        "session_evals" => {
            let (ids, evals) = pack_batch_payloads::<crate::eval::types::EvalRow>(rows, cfg, kind)?;
            if ids.is_empty() {
                return Ok(None);
            }
            Ok(Some((
                ids,
                IngestExportBatch::SessionEvals(crate::sync::export_batch::SessionEvalsBatchBody {
                    evals,
                }),
            )))
        }
        _ => Ok(None),
    }
}

fn pack_batch_payloads<T>(
    rows: &[(i64, String, String)],
    cfg: &crate::core::config::SyncConfig,
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

/// Primary POST in parallel with optional exporter fan-out. Returns `(post, fan)`.
fn post_with_fanout(
    client: &SyncHttpClient,
    body: &IngestExportBatch,
    key: &Uuid,
    flush: &FlushExporters<'_>,
) -> Result<(
    Result<PostBatchOutcome, anyhow::Error>,
    Result<(), anyhow::Error>,
)> {
    let fan_body = body.clone();
    let reg = flush.registry;
    let fail_open = flush.telemetry.fail_open;
    Ok(std::thread::scope(|s| {
        let handle = s.spawn(move || {
            if let Some(r) = reg {
                r.fan_out(fail_open, &fan_body)
            } else {
                Ok(())
            }
        });
        let post_res: Result<PostBatchOutcome, anyhow::Error> = (|| {
            let o = match body {
                IngestExportBatch::Events(b) => client.post_events_batch(b, key)?,
                IngestExportBatch::ToolSpans(b) => client.post_tool_spans_batch(b, key)?,
                IngestExportBatch::RepoSnapshots(b) => client.post_repo_snapshots_batch(b, key)?,
                IngestExportBatch::WorkspaceFacts(b) => {
                    client.post_workspace_facts_batch(b, key)?
                }
                IngestExportBatch::SessionEvals(b) => client.post_session_evals_batch(b, key)?,
            };
            Ok(o)
        })();
        let fan_res = match handle.join() {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(e),
            Err(p) => Err(anyhow::anyhow!("telemetry fan-out join panicked: {p:?}")),
        };
        (post_res, fan_res)
    }))
}

fn post_batch_resilient(
    client: &SyncHttpClient,
    store: &Store,
    body: IngestExportBatch,
    ids: &[i64],
    flush: &FlushExporters<'_>,
) -> Result<Sent> {
    let mut backoff = Duration::from_millis(200);
    let max_backoff = Duration::from_secs(30);
    let mut server_failures = 0u32;

    loop {
        if body.item_count() == 0 {
            return Ok(Sent::default());
        }

        let key = Uuid::now_v7();
        let (post_res, fan_res) = post_with_fanout(client, &body, &key, flush)?;
        let outcome = post_res;

        let outcome = match outcome {
            Ok(o) => o,
            Err(e) => {
                if fan_res.is_err() {
                    tracing::trace!(error = %e, "primary post and fan-out both failed");
                }
                return Err(e);
            }
        };

        match outcome {
            PostBatchOutcome::Accepted { .. } | PostBatchOutcome::Conflict => {
                if let Err(e) = fan_res {
                    return Err(
                        e.context("telemetry fan-out (before outbox commit; fail_open = false)")
                    );
                }
                store.mark_outbox_sent(ids)?;
                store.set_sync_state_ok()?;
                return Ok(Sent {
                    batches: 1,
                    events: ids.len() as u64,
                });
            }
            PostBatchOutcome::TooLarge => {
                if let Err(e) = fan_res {
                    tracing::warn!(error = %e, "telemetry fan-out failed; continuing 413 split");
                }
                if body.item_count() <= 1 {
                    store.set_sync_state_error("413: single event too large for server")?;
                    anyhow::bail!(
                        "413: single event too large; tighten redaction or max_body_bytes"
                    );
                }
                let mid = body.item_count() / 2;
                let left_ids = ids[..mid].to_vec();
                let right_ids = ids[mid..].to_vec();
                let (left_body, right_body) = split_batch(body, mid);
                let a = post_batch_resilient(client, store, left_body, &left_ids, flush)?;
                let b = post_batch_resilient(client, store, right_body, &right_ids, flush)?;
                return Ok(Sent {
                    batches: a.batches + b.batches,
                    events: a.events + b.events,
                });
            }
            PostBatchOutcome::RateLimited(d) => {
                if let Err(e) = fan_res {
                    tracing::warn!(error = %e, "telemetry fan-out failed during 429; will retry");
                }
                thread::sleep(d);
            }
            PostBatchOutcome::Unauthorized => {
                if let Err(e) = fan_res {
                    tracing::warn!(error = %e, "telemetry fan-out during 401");
                }
                let msg = "401 unauthorized (check team_token)";
                store.set_sync_state_error(msg)?;
                anyhow::bail!("{msg}");
            }
            PostBatchOutcome::ClientError(c) => {
                if let Err(e) = fan_res {
                    tracing::warn!(error = %e, "telemetry fan-out during client error {c}");
                }
                let msg = format!("HTTP client error {c}");
                store.set_sync_state_error(&msg)?;
                anyhow::bail!("{msg}");
            }
            PostBatchOutcome::ServerError(c) => {
                if let Err(e) = fan_res {
                    tracing::warn!(error = %e, "telemetry fan-out during {c} server error");
                }
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

fn split_batch(body: IngestExportBatch, mid: usize) -> (IngestExportBatch, IngestExportBatch) {
    match body {
        IngestExportBatch::Events(body) => (
            IngestExportBatch::Events(EventsBatchBody {
                team_id: body.team_id.clone(),
                workspace_hash: body.workspace_hash.clone(),
                events: body.events[..mid].to_vec(),
            }),
            IngestExportBatch::Events(EventsBatchBody {
                team_id: body.team_id,
                workspace_hash: body.workspace_hash,
                events: body.events[mid..].to_vec(),
            }),
        ),
        IngestExportBatch::ToolSpans(body) => (
            IngestExportBatch::ToolSpans(ToolSpansBatchBody {
                team_id: body.team_id.clone(),
                workspace_hash: body.workspace_hash.clone(),
                spans: body.spans[..mid].to_vec(),
            }),
            IngestExportBatch::ToolSpans(ToolSpansBatchBody {
                team_id: body.team_id,
                workspace_hash: body.workspace_hash,
                spans: body.spans[mid..].to_vec(),
            }),
        ),
        IngestExportBatch::RepoSnapshots(body) => (
            IngestExportBatch::RepoSnapshots(RepoSnapshotsBatchBody {
                team_id: body.team_id.clone(),
                workspace_hash: body.workspace_hash.clone(),
                snapshots: body.snapshots[..mid].to_vec(),
            }),
            IngestExportBatch::RepoSnapshots(RepoSnapshotsBatchBody {
                team_id: body.team_id,
                workspace_hash: body.workspace_hash,
                snapshots: body.snapshots[mid..].to_vec(),
            }),
        ),
        IngestExportBatch::WorkspaceFacts(body) => (
            IngestExportBatch::WorkspaceFacts(WorkspaceFactsBatchBody {
                team_id: body.team_id.clone(),
                workspace_hash: body.workspace_hash.clone(),
                facts: body.facts[..mid].to_vec(),
            }),
            IngestExportBatch::WorkspaceFacts(WorkspaceFactsBatchBody {
                team_id: body.team_id,
                workspace_hash: body.workspace_hash,
                facts: body.facts[mid..].to_vec(),
            }),
        ),
        IngestExportBatch::SessionEvals(body) => (
            IngestExportBatch::SessionEvals(crate::sync::export_batch::SessionEvalsBatchBody {
                evals: body.evals[..mid].to_vec(),
            }),
            IngestExportBatch::SessionEvals(crate::sync::export_batch::SessionEvalsBatchBody {
                evals: body.evals[mid..].to_vec(),
            }),
        ),
    }
}
