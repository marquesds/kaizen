// SPDX-License-Identifier: AGPL-3.0-or-later
//! Scientific guidance for skills and Cursor rules.

pub mod inventory;
pub mod llm;
pub mod proposals;
pub mod score;
mod score_inputs;
mod score_math;
pub mod types;
pub mod validation;

pub use types::{
    Artifact, ArtifactKind, ArtifactRef, ArtifactState, CandidateAction, CandidateStatus,
    GuidanceCandidate, GuidanceScoreReport, GuidanceScoreRow, GuidanceScoreSlice,
    GuidanceValidationGate,
};
