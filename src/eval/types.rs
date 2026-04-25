// SPDX-License-Identifier: AGPL-3.0-or-later
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalRow {
    pub id: String,
    pub session_id: String,
    pub judge_model: String,
    pub rubric_id: String,
    pub score: f64,
    pub rationale: String,
    pub flagged: bool,
    pub created_at_ms: u64,
}

#[derive(Debug, Deserialize)]
pub struct JudgeResponse {
    pub score: f64,
    pub rationale: String,
}
