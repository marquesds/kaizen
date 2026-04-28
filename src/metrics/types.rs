// SPDX-License-Identifier: AGPL-3.0-or-later
//! Smart metric data. Pure structs only.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SymbolFact {
    pub path: String,
    pub name: String,
    pub kind: String,
    pub complexity: u32,
    pub calls: Vec<String>,
    pub start_byte: usize,
    pub end_byte: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileFact {
    pub snapshot_id: String,
    pub path: String,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepoEdge {
    pub from_path: String,
    pub to_path: String,
    pub kind: String,
    pub weight: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepoSnapshotRecord {
    pub id: String,
    pub workspace: String,
    pub head_commit: Option<String>,
    pub dirty_fingerprint: String,
    pub analyzer_version: String,
    pub indexed_at_ms: u64,
    pub dirty: bool,
    pub graph_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolSpanView {
    pub span_id: String,
    pub tool: String,
    pub status: String,
    pub lead_time_ms: Option<u64>,
    pub tokens_in: Option<u32>,
    pub tokens_out: Option<u32>,
    pub reasoning_tokens: Option<u32>,
    pub cost_usd_e6: Option<i64>,
    pub paths: Vec<String>,
    pub parent_span_id: Option<String>,
    pub depth: u32,
    pub subtree_cost_usd_e6: Option<i64>,
    pub subtree_token_count: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolSpanSample {
    pub span_id: String,
    pub session_id: String,
    pub tool: Option<String>,
    pub lead_time_ms: Option<u64>,
    pub tokens_in: Option<u32>,
    pub tokens_out: Option<u32>,
    pub reasoning_tokens: Option<u32>,
    pub cost_usd_e6: Option<i64>,
    pub paths: Vec<String>,
}

impl From<&crate::store::tool_span_index::ToolSpanRecord> for ToolSpanSample {
    fn from(span: &crate::store::tool_span_index::ToolSpanRecord) -> Self {
        Self {
            span_id: span.span_id.clone(),
            session_id: span.session_id.clone(),
            tool: span.tool.clone(),
            lead_time_ms: span.lead_time_ms,
            tokens_in: span.tokens_in,
            tokens_out: span.tokens_out,
            reasoning_tokens: span.reasoning_tokens,
            cost_usd_e6: span.cost_usd_e6,
            paths: span.paths.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RankedFile {
    pub path: String,
    pub value: u64,
    pub complexity_total: u32,
    pub churn_30d: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RankedTool {
    pub tool: String,
    pub calls: u64,
    pub p50_ms: Option<u64>,
    pub p95_ms: Option<u64>,
    pub total_tokens: u64,
    pub total_reasoning_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MetricsReport {
    pub snapshot: Option<RepoSnapshotRecord>,
    pub hottest_files: Vec<RankedFile>,
    pub most_changed_files: Vec<RankedFile>,
    pub most_complex_files: Vec<RankedFile>,
    pub highest_risk_files: Vec<RankedFile>,
    pub slowest_tools: Vec<RankedTool>,
    pub highest_token_tools: Vec<RankedTool>,
    pub highest_reasoning_tools: Vec<RankedTool>,
    pub agent_pain_hotspots: Vec<RankedFile>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileHistory {
    pub churn_30d: u32,
    pub churn_90d: u32,
    pub authors_90d: u32,
    pub last_changed_ms: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RepoAnalysis {
    pub path: String,
    pub language: String,
    pub bytes: u64,
    pub loc: u32,
    pub sloc: u32,
    pub complexity_total: u32,
    pub max_fn_complexity: u32,
    pub imports: Vec<String>,
    pub symbols: Vec<SymbolFact>,
}
