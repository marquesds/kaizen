// SPDX-License-Identifier: AGPL-3.0-or-later
//! Pure data for the retro engine (`Report`, `Bet`, `Inputs`).

use crate::core::event::{Event, SessionRecord};
use crate::feedback::types::FeedbackRecord;
use crate::metrics::types::{FileFact, ToolSpanView};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Workspace-local facts assembled once at the IO boundary.
#[derive(Debug, Clone)]
pub struct Inputs {
    pub window_start_ms: u64,
    pub window_end_ms: u64,
    /// Joined rows time-ordered.
    pub events: Vec<(SessionRecord, Event)>,
    pub files_touched: Vec<(String, String)>,
    pub skills_used: Vec<(String, String)>,
    pub tool_spans: Vec<ToolSpanView>,
    /// Skills referenced in the last `usage_lookback_ms` window (for H1).
    pub skills_used_recent_slugs: HashSet<String>,
    pub usage_lookback_ms: u64,
    pub skill_files_on_disk: Vec<SkillFileOnDisk>,
    /// `.cursor/rules/*.mdc` stems (same shape as [`SkillFileOnDisk`]).
    pub rule_files_on_disk: Vec<SkillFileOnDisk>,
    pub rules_used_recent_slugs: HashSet<String>,
    pub file_facts: HashMap<String, FileFact>,
    pub aggregates: RetroAggregates,
    /// LLM-as-Judge eval scores for sessions in the window: (session_id, score 0..1).
    pub eval_scores: Vec<(String, f64)>,
    /// Sessions with a recorded prompt fingerprint: (session_id, fingerprint).
    pub prompt_fingerprints: Vec<(String, String)>,
    /// Human feedback records in the window.
    pub feedback: Vec<FeedbackRecord>,
}

#[derive(Debug, Clone)]
pub struct SkillFileOnDisk {
    pub slug: String,
    /// Bytes of frontmatter + body (rough token proxy).
    pub size_bytes: u64,
    pub mtime_ms: u64,
}

#[derive(Debug, Clone, Default)]
pub struct RetroAggregates {
    pub unique_session_ids: HashSet<String>,
    pub tool_event_counts: HashMap<String, u64>,
    pub tool_cost_usd_e6: HashMap<String, i64>,
    pub model_session_counts: HashMap<String, u64>,
    pub total_cost_usd_e6: i64,
    pub span_tree_stats: Option<SpanTreeStats>,
}

#[derive(Debug, Clone)]
pub struct SpanTreeStats {
    pub max_depth: u32,
    pub max_fan_out: u32,
    pub deepest_span_id: String,
}

/// One ranked improvement bet.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Bet {
    /// Stable id for dedup (`H2:foo.rs|bar.rs`).
    pub id: String,
    pub heuristic_id: String,
    pub title: String,
    pub hypothesis: String,
    pub expected_tokens_saved_per_week: f64,
    pub effort_minutes: u32,
    pub evidence: Vec<String>,
    pub apply_step: String,
    #[serde(default)]
    pub evidence_recency_ms: u64,
}

impl Bet {
    pub fn score(&self) -> f64 {
        self.expected_tokens_saved_per_week / (self.effort_minutes as f64 + 1.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RetroMeta {
    pub week_label: String,
    pub span_start_ms: u64,
    pub span_end_ms: u64,
    pub session_count: u64,
    pub total_cost_usd_e6: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RetroStats {
    pub sessions: u64,
    pub total_cost_usd_e6: i64,
    pub top_model: Option<String>,
    pub top_model_pct: Option<u64>,
    pub top_tool: Option<String>,
    pub top_tool_pct: Option<u64>,
    pub median_session_minutes: Option<u64>,
}

/// JSON + markdown source of truth for CLI `--json` and reports.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Report {
    pub meta: RetroMeta,
    pub top_bets: Vec<Bet>,
    pub skipped_deduped: Vec<String>,
    pub stats: RetroStats,
}
