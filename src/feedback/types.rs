// SPDX-License-Identifier: AGPL-3.0-or-later
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum FeedbackLabel {
    Good,
    Bad,
    Interesting,
    Bug,
    Regression,
}

impl FeedbackLabel {
    pub fn from_str_opt(s: &str) -> Option<Self> {
        match s {
            "good" => Some(Self::Good),
            "bad" => Some(Self::Bad),
            "interesting" => Some(Self::Interesting),
            "bug" => Some(Self::Bug),
            "regression" => Some(Self::Regression),
            _ => None,
        }
    }

    pub fn to_db_str(&self) -> &str {
        match self {
            Self::Good => "good",
            Self::Bad => "bad",
            Self::Interesting => "interesting",
            Self::Bug => "bug",
            Self::Regression => "regression",
        }
    }
}

impl fmt::Display for FeedbackLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_db_str())
    }
}

/// Score in range 1..=5.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeedbackScore(pub u8);

impl FeedbackScore {
    /// Returns `None` if `v` is not in 1..=5.
    pub fn new(v: u8) -> Option<Self> {
        if (1..=5).contains(&v) {
            Some(Self(v))
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeedbackRecord {
    pub id: String,
    pub session_id: String,
    pub score: Option<FeedbackScore>,
    pub label: Option<FeedbackLabel>,
    pub note: Option<String>,
    pub created_at_ms: u64,
}
