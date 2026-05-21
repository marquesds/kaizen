// SPDX-License-Identifier: AGPL-3.0-or-later
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceHit {
    pub session_id: String,
    pub seq: Option<u64>,
    pub ts_ms: u64,
    pub agent: String,
    pub kind: String,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaseRecord {
    pub id: String,
    pub source_key: String,
    pub session_id: String,
    pub reason: String,
    pub label: Option<String>,
    pub status: CaseStatus,
    pub prompt_fingerprint: Option<String>,
    pub metadata_json: String,
    pub created_at_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CaseStatus {
    Open,
    Archived,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaseRef {
    pub case_id: String,
    pub ref_kind: String,
    pub ref_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalRule {
    pub id: String,
    pub name: String,
    pub filter: String,
    pub action: RuleAction,
    pub enabled: bool,
    pub created_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RuleAction {
    CreateCase { label: Option<String> },
    QueueReview { title: Option<String> },
    EmitAlert { severity: AlertSeverity },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlertEvent {
    pub id: String,
    pub source_key: String,
    pub name: String,
    pub severity: AlertSeverity,
    pub message: String,
    pub session_id: Option<String>,
    pub created_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewItem {
    pub id: String,
    pub source_key: String,
    pub session_id: String,
    pub title: String,
    pub status: ReviewStatus,
    pub created_at_ms: u64,
    pub resolved_at_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReviewStatus {
    Open,
    Resolved,
    Dismissed,
}

impl CaseStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Archived => "archived",
        }
    }
}

impl ReviewStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Resolved => "resolved",
            Self::Dismissed => "dismissed",
        }
    }
}

impl AlertSeverity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Critical => "critical",
        }
    }
}
