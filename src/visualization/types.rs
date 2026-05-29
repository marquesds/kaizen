// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::core::event::{Event, SessionRecord};
use crate::store::SpanNode;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct VisualizationReport {
    pub generated_at_ms: u64,
    pub workspace: String,
    pub totals: VisualizationTotals,
    pub activity: ActivityReport,
    pub sessions: Vec<TraceSummary>,
    pub selected: Option<TraceDetail>,
    pub quality: DataQuality,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct VisualizationTotals {
    pub session_count: u64,
    pub running_count: u64,
    pub event_count: u64,
    pub error_count: u64,
    pub tool_call_count: u64,
    pub cost_usd_e6: i64,
    pub tokens: TokenTotals,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct TokenTotals {
    pub input: u64,
    pub output: u64,
    pub reasoning: u64,
    pub cache_read: u64,
    pub cache_create: u64,
    pub total: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ActivityReport {
    pub metric: ActivityMetric,
    pub day_bins: Vec<ActivityBin>,
    pub week_bins: Vec<ActivityBin>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityMetric {
    #[default]
    Events,
    Sessions,
    Tokens,
    Cost,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ActivityBin {
    pub start_ms: u64,
    pub end_ms: u64,
    pub event_count: u64,
    pub session_count: u64,
    pub token_total: u64,
    pub cost_usd_e6: i64,
    pub active_by_agent: Vec<(String, u64)>,
    pub dominant_agent: Option<String>,
    pub dominant_kind: Option<String>,
    pub heat: f64,
    pub is_break: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TraceSummary {
    pub id: String,
    pub agent: String,
    pub model: Option<String>,
    pub status: DerivedStatus,
    pub status_reason: String,
    pub started_at_ms: u64,
    pub ended_at_ms: Option<u64>,
    pub last_event_ms: Option<u64>,
    pub event_count: u64,
    pub error_count: u64,
    pub tool_call_count: u64,
    pub cost_usd_e6: i64,
    pub tokens: TokenTotals,
    pub top_tools: Vec<(String, u64)>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TraceDetail {
    pub session: SessionRecord,
    pub events: Vec<Event>,
    pub spans: Vec<SpanNode>,
    pub files: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DataQuality {
    pub token_coverage_pct: f64,
    pub cost_coverage_pct: f64,
    pub partial_cost_sessions: u64,
    pub adapter_errors: Vec<String>,
    pub stale_scan: bool,
    pub warnings: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DerivedStatus {
    Active,
    WaitingOnTool,
    Idle,
    Done,
    Errored,
    Orphaned,
}
