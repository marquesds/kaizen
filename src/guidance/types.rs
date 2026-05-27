// SPDX-License-Identifier: AGPL-3.0-or-later
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    Skill,
    Rule,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactState {
    Current,
    Stale,
    InsufficientEvidence,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct ArtifactRef {
    pub kind: ArtifactKind,
    pub slug: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Artifact {
    pub kind: ArtifactKind,
    pub slug: String,
    pub path: PathBuf,
    pub content_hash: String,
    pub bytes: u64,
    pub mtime_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GuidanceScoreRow {
    pub artifact: ArtifactRef,
    pub path: String,
    pub state: ArtifactState,
    pub score: f64,
    pub sessions: u64,
    pub avg_cost_usd: Option<f64>,
    pub mean_eval_score: Option<f64>,
    pub bad_feedback: u64,
    pub failed_outcomes: u64,
    pub tool_loops: u64,
    pub train: GuidanceScoreSlice,
    pub validation: GuidanceScoreSlice,
    pub generalization_gap: Option<f64>,
    pub validation_gate: GuidanceValidationGate,
    pub evidence: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct GuidanceScoreSlice {
    pub score: f64,
    pub sessions: u64,
    pub avg_cost_usd: Option<f64>,
    pub mean_eval_score: Option<f64>,
    pub bad_feedback: u64,
    pub failed_outcomes: u64,
    pub tool_loops: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuidanceValidationGate {
    #[default]
    NoData,
    NeedsMoreValidation,
    Stable,
    Regression,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GuidanceScoreReport {
    pub workspace: String,
    pub window_start_ms: u64,
    pub window_end_ms: u64,
    pub validation_start_ms: u64,
    pub min_sessions: u64,
    pub rows: Vec<GuidanceScoreRow>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateStatus {
    Proposed,
    Applied,
    Validated,
    Rejected,
    Archived,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "op")]
pub enum CandidateAction {
    Delete,
    Replace { content: String },
    ReviewOnly,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GuidanceCandidate {
    pub id: String,
    pub artifact: ArtifactRef,
    pub action: CandidateAction,
    pub status: CandidateStatus,
    pub rationale: String,
    pub evidence: Vec<String>,
    pub created_at_ms: u64,
    pub applied_at_ms: Option<u64>,
    pub treatment_fingerprint: Option<String>,
    pub experiment_id: Option<String>,
    pub backup_path: Option<String>,
}

impl ArtifactRef {
    pub fn parse(raw: &str) -> Option<Self> {
        let (kind, slug) = raw.split_once(':')?;
        Some(Self {
            kind: ArtifactKind::parse(kind)?,
            slug: slug.to_string(),
        })
    }
}

impl ArtifactKind {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "skill" => Some(Self::Skill),
            "rule" => Some(Self::Rule),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Skill => "skill",
            Self::Rule => "rule",
        }
    }
}

impl CandidateStatus {
    pub fn parse(raw: &str) -> Option<Self> {
        Some(match raw {
            "proposed" => Self::Proposed,
            "applied" => Self::Applied,
            "validated" => Self::Validated,
            "rejected" => Self::Rejected,
            "archived" => Self::Archived,
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Proposed => "proposed",
            Self::Applied => "applied",
            Self::Validated => "validated",
            Self::Rejected => "rejected",
            Self::Archived => "archived",
        }
    }
}

impl fmt::Display for ArtifactRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.kind.as_str(), self.slug)
    }
}
