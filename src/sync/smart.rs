// SPDX-License-Identifier: AGPL-3.0-or-later
//! Sync shapes for tool spans and repo snapshots.

use crate::core::config::try_team_salt;
use crate::metrics::types::{FileFact, RepoEdge, RepoSnapshotRecord};
use crate::store::{Store, sqlite::ToolSpanSyncRow};
use crate::sync::context::SyncIngestContext;
use crate::sync::outbound::{hash_with_salt, workspace_hash};
use anyhow::Result;
use serde::{Deserialize, Serialize};

const SNAPSHOT_CHUNK: usize = 100;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpansBatchBody {
    pub team_id: String,
    pub workspace_hash: String,
    pub spans: Vec<OutboundToolSpan>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboundToolSpan {
    pub session_id_hash: String,
    pub span_id_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    pub status: String,
    pub started_at_ms: Option<u64>,
    pub ended_at_ms: Option<u64>,
    pub lead_time_ms: Option<u64>,
    pub tokens_in: Option<u32>,
    pub tokens_out: Option<u32>,
    pub reasoning_tokens: Option<u32>,
    pub cost_usd_e6: Option<i64>,
    pub path_hashes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoSnapshotsBatchBody {
    pub team_id: String,
    pub workspace_hash: String,
    pub snapshots: Vec<OutboundRepoSnapshotChunk>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboundRepoSnapshotChunk {
    pub snapshot_id_hash: String,
    pub commit_hash: Option<String>,
    pub indexed_at_ms: u64,
    pub dirty: bool,
    pub chunk_index: u32,
    pub chunk_total: u32,
    pub file_facts: Vec<OutboundFileFact>,
    pub edges: Vec<OutboundRepoEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboundFileFact {
    pub path_hash: String,
    pub language: String,
    pub bytes: u64,
    pub loc: u32,
    pub sloc: u32,
    pub complexity_total: u32,
    pub max_fn_complexity: u32,
    pub symbol_count: u32,
    pub import_count: u32,
    pub fan_in: u32,
    pub fan_out: u32,
    pub churn_30d: u32,
    pub churn_90d: u32,
    pub authors_90d: u32,
    pub last_changed_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboundRepoEdge {
    pub from_hash: String,
    pub to_hash: String,
    pub kind: String,
    pub weight: u32,
}

pub fn enqueue_tool_spans_for_session(
    store: &Store,
    session_id: &str,
    ctx: &SyncIngestContext,
) -> Result<()> {
    let Some(salt) = try_team_salt(&ctx.sync) else {
        return Ok(());
    };
    let rows = store.tool_spans_for_session(session_id)?;
    let payloads = rows
        .iter()
        .map(|row| serde_json::to_string(&outbound_tool_span(row, &salt)))
        .collect::<serde_json::Result<Vec<_>>>()?;
    store.replace_outbox_rows(session_id, "tool_spans", &payloads)
}

pub fn enqueue_repo_snapshot(
    store: &Store,
    snapshot: &RepoSnapshotRecord,
    facts: &[FileFact],
    edges: &[RepoEdge],
    ctx: &SyncIngestContext,
) -> Result<()> {
    let Some(salt) = try_team_salt(&ctx.sync) else {
        return Ok(());
    };
    let chunks = outbound_snapshot_chunks(snapshot, facts, edges, &salt);
    let payloads = chunks
        .iter()
        .map(serde_json::to_string)
        .collect::<serde_json::Result<Vec<_>>>()?;
    store.replace_outbox_rows(&snapshot.id, "repo_snapshots", &payloads)
}

pub fn outbound_tool_span(row: &ToolSpanSyncRow, salt: &[u8; 32]) -> OutboundToolSpan {
    OutboundToolSpan {
        session_id_hash: hash_with_salt(salt, row.session_id.as_bytes()),
        span_id_hash: hash_with_salt(salt, row.span_id.as_bytes()),
        tool: row.tool.clone(),
        status: row.status.clone(),
        started_at_ms: row.started_at_ms,
        ended_at_ms: row.ended_at_ms,
        lead_time_ms: row.lead_time_ms,
        tokens_in: row.tokens_in,
        tokens_out: row.tokens_out,
        reasoning_tokens: row.reasoning_tokens,
        cost_usd_e6: row.cost_usd_e6,
        path_hashes: row
            .paths
            .iter()
            .map(|path| hash_with_salt(salt, format!("path:{path}").as_bytes()))
            .collect(),
    }
}

pub fn outbound_snapshot_chunks(
    snapshot: &RepoSnapshotRecord,
    facts: &[FileFact],
    edges: &[RepoEdge],
    salt: &[u8; 32],
) -> Vec<OutboundRepoSnapshotChunk> {
    let fact_chunks = facts.chunks(SNAPSHOT_CHUNK).collect::<Vec<_>>();
    let edge_chunks = edges.chunks(SNAPSHOT_CHUNK).collect::<Vec<_>>();
    let total = fact_chunks.len().max(edge_chunks.len()).max(1) as u32;
    (0..total)
        .map(|idx| OutboundRepoSnapshotChunk {
            snapshot_id_hash: hash_with_salt(salt, snapshot.id.as_bytes()),
            commit_hash: snapshot
                .head_commit
                .as_ref()
                .map(|commit| hash_with_salt(salt, format!("commit:{commit}").as_bytes())),
            indexed_at_ms: snapshot.indexed_at_ms,
            dirty: snapshot.dirty,
            chunk_index: idx,
            chunk_total: total,
            file_facts: fact_chunks
                .get(idx as usize)
                .map(|chunk| chunk.iter().map(|fact| outbound_fact(fact, salt)).collect())
                .unwrap_or_default(),
            edges: edge_chunks
                .get(idx as usize)
                .map(|chunk| chunk.iter().map(|edge| outbound_edge(edge, salt)).collect())
                .unwrap_or_default(),
        })
        .collect()
}

pub fn workspace_hash_for(ctx: &SyncIngestContext) -> Option<String> {
    let salt = try_team_salt(&ctx.sync)?;
    Some(workspace_hash(&salt, ctx.workspace_root()))
}

fn outbound_fact(fact: &FileFact, salt: &[u8; 32]) -> OutboundFileFact {
    OutboundFileFact {
        path_hash: hash_with_salt(salt, format!("path:{}", fact.path).as_bytes()),
        language: fact.language.clone(),
        bytes: fact.bytes,
        loc: fact.loc,
        sloc: fact.sloc,
        complexity_total: fact.complexity_total,
        max_fn_complexity: fact.max_fn_complexity,
        symbol_count: fact.symbol_count,
        import_count: fact.import_count,
        fan_in: fact.fan_in,
        fan_out: fact.fan_out,
        churn_30d: fact.churn_30d,
        churn_90d: fact.churn_90d,
        authors_90d: fact.authors_90d,
        last_changed_ms: fact.last_changed_ms,
    }
}

fn outbound_edge(edge: &RepoEdge, salt: &[u8; 32]) -> OutboundRepoEdge {
    OutboundRepoEdge {
        from_hash: hash_with_salt(salt, format!("graph:{}", edge.from_path).as_bytes()),
        to_hash: hash_with_salt(salt, format!("graph:{}", edge.to_path).as_bytes()),
        kind: edge.kind.clone(),
        weight: edge.weight,
    }
}
