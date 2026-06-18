// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::core::event::{SessionRecord, SessionStatus};

/// Per-workspace activity dashboard stats.
#[derive(Clone)]
pub struct InsightsStats {
    pub total_sessions: u64,
    pub running_sessions: u64,
    pub total_events: u64,
    /// (day label e.g. "Mon", count) last 7 days oldest first
    pub sessions_by_day: Vec<(String, u64)>,
    /// Recent sessions DESC by started_at, max 3; paired with event count
    pub recent: Vec<(SessionRecord, u64)>,
    /// Top tools by event count, max 5
    pub top_tools: Vec<(String, u64)>,
    pub total_cost_usd_e6: i64,
    pub sessions_with_cost: u64,
}

/// Sync daemon / outbox status for `kaizen sync status`.
pub struct SyncStatusSnapshot {
    pub pending_outbox: u64,
    pub last_success_ms: Option<u64>,
    pub last_error: Option<String>,
    pub consecutive_failures: u32,
}

/// Aggregate stats across sessions + events for a workspace.
#[derive(serde::Serialize)]
pub struct SummaryStats {
    pub session_count: u64,
    pub total_cost_usd_e6: i64,
    pub by_agent: Vec<(String, u64)>,
    pub by_model: Vec<(String, u64)>,
    pub top_tools: Vec<(String, u64)>,
}

/// Skill vs Cursor rule for [`GuidancePerfRow`].
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GuidanceKind {
    Skill,
    Rule,
}

/// One row for `kaizen guidance` — observed references in payloads (not Cursor auto-load counts).
#[derive(Clone, Debug, serde::Serialize)]
pub struct GuidancePerfRow {
    pub kind: GuidanceKind,
    pub id: String,
    pub sessions: u64,
    pub sessions_pct: f64,
    pub total_cost_usd_e6: i64,
    pub avg_cost_per_session_usd: Option<f64>,
    pub vs_workspace_avg_cost_per_session_usd: Option<f64>,
    pub on_disk: bool,
}

/// Aggregated skill/rule adoption and cost proxy for a time window.
#[derive(Clone, Debug, serde::Serialize)]
pub struct GuidanceReport {
    pub workspace: String,
    pub window_start_ms: u64,
    pub window_end_ms: u64,
    pub sessions_in_window: u64,
    pub workspace_avg_cost_per_session_usd: Option<f64>,
    pub rows: Vec<GuidancePerfRow>,
}

/// Result of [`super::Store::prune_sessions_started_before`].
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub struct PruneStats {
    pub sessions_removed: u64,
    pub events_removed: u64,
}

/// Row in `session_outcomes` (Tier C — post-stop test/lint snapshot).
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SessionOutcomeRow {
    pub session_id: String,
    pub test_passed: Option<i64>,
    pub test_failed: Option<i64>,
    pub test_skipped: Option<i64>,
    pub build_ok: Option<bool>,
    pub lint_errors: Option<i64>,
    pub revert_lines_14d: Option<i64>,
    pub pr_open: Option<i64>,
    pub ci_ok: Option<bool>,
    pub measured_at_ms: u64,
    pub measure_error: Option<String>,
}

/// Aggregated process samples for retro (Tier D).
#[derive(Debug, Clone)]
pub struct SessionSampleAgg {
    pub session_id: String,
    pub sample_count: u64,
    pub max_cpu_percent: f64,
    pub max_rss_bytes: u64,
}

pub struct ToolSpanSyncRow {
    pub span_id: String,
    pub session_id: String,
    pub tool: Option<String>,
    pub tool_call_id: Option<String>,
    pub status: String,
    pub started_at_ms: Option<u64>,
    pub ended_at_ms: Option<u64>,
    pub lead_time_ms: Option<u64>,
    pub tokens_in: Option<u32>,
    pub tokens_out: Option<u32>,
    pub reasoning_tokens: Option<u32>,
    pub cost_usd_e6: Option<i64>,
    pub paths: Vec<String>,
}

pub(crate) struct CaptureQualityRow {
    pub source: String,
    pub has_tokens: bool,
    pub has_cost: bool,
    pub has_latency: bool,
    pub has_context: bool,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
}

pub(crate) struct TraceSpanQualityRow {
    pub kind: String,
    pub is_orphan: bool,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum StoreOpenMode {
    ReadWrite,
    ReadOnlyQuery,
}

#[derive(Debug, Clone)]
pub struct SessionStatusRow {
    pub id: String,
    pub status: SessionStatus,
    pub ended_at_ms: Option<u64>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct SessionFilter {
    pub agent_prefix: Option<String>,
    pub status: Option<SessionStatus>,
    pub since_ms: Option<u64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SessionPage {
    pub rows: Vec<SessionRecord>,
    pub total: usize,
    pub next_offset: Option<usize>,
}
