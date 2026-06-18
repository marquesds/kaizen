// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::guidance::types::{ArtifactState, CandidateAction, CandidateStatus};
use crate::guidance::types::{GuidanceCandidate, GuidanceScoreRow};

pub fn deterministic(
    row: &GuidanceScoreRow,
    now_ms: u64,
    rejected: &[GuidanceCandidate],
) -> GuidanceCandidate {
    let intended = intended_action(row);
    let blocked = was_rejected(&intended, rejected);
    let action = if blocked {
        CandidateAction::ReviewOnly
    } else {
        intended
    };
    GuidanceCandidate {
        id: uuid::Uuid::now_v7().to_string(),
        artifact: row.artifact.clone(),
        action,
        status: CandidateStatus::Proposed,
        rationale: rationale(row, blocked),
        evidence: evidence(row, rejected),
        created_at_ms: now_ms,
        applied_at_ms: None,
        treatment_fingerprint: None,
        experiment_id: None,
        backup_path: None,
    }
}

pub fn rejected_memory_lines(rejected: &[GuidanceCandidate]) -> Vec<String> {
    rejected
        .iter()
        .take(5)
        .map(|c| format!("{} {}: {}", c.id, action_label(&c.action), c.rationale))
        .collect()
}

fn intended_action(row: &GuidanceScoreRow) -> CandidateAction {
    match row.state {
        ArtifactState::Stale => CandidateAction::Delete,
        _ => CandidateAction::ReviewOnly,
    }
}

fn rationale(row: &GuidanceScoreRow, blocked: bool) -> String {
    if blocked {
        return "prior equivalent candidate rejected; review manually".into();
    }
    match row.state {
        ArtifactState::Stale => "artifact has no observed sessions in window".into(),
        ArtifactState::Current => format!(
            "artifact score {:.1}; review evidence before editing",
            row.score
        ),
        ArtifactState::InsufficientEvidence => "not enough sessions for validation".into(),
    }
}

fn evidence(row: &GuidanceScoreRow, rejected: &[GuidanceCandidate]) -> Vec<String> {
    row.evidence
        .iter()
        .cloned()
        .chain(rejected_memory_lines(rejected))
        .collect()
}

fn was_rejected(action: &CandidateAction, rejected: &[GuidanceCandidate]) -> bool {
    rejected.iter().any(|c| same_action(action, &c.action))
}

fn same_action(a: &CandidateAction, b: &CandidateAction) -> bool {
    match (a, b) {
        (CandidateAction::Delete, CandidateAction::Delete) => true,
        (CandidateAction::ReviewOnly, CandidateAction::ReviewOnly) => true,
        (CandidateAction::Replace { content: x }, CandidateAction::Replace { content: y }) => {
            x == y
        }
        _ => false,
    }
}

pub fn action_label(action: &CandidateAction) -> &'static str {
    match action {
        CandidateAction::Delete => "delete",
        CandidateAction::Replace { .. } => "replace",
        CandidateAction::ReviewOnly => "review_only",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::guidance::{
        ArtifactKind, ArtifactRef, ArtifactState, CandidateStatus, GuidanceCandidate,
    };

    #[test]
    fn rejected_delete_blocks_repeated_delete() {
        let c = deterministic(&row(), 1, &[candidate(CandidateAction::Delete)]);
        assert_eq!(c.action, CandidateAction::ReviewOnly);
        assert!(c.rationale.contains("rejected"));
    }

    fn row() -> GuidanceScoreRow {
        GuidanceScoreRow {
            artifact: ArtifactRef {
                kind: ArtifactKind::Rule,
                slug: "dead".into(),
            },
            path: "dead".into(),
            state: ArtifactState::Stale,
            score: 0.0,
            sessions: 0,
            avg_cost_usd: None,
            mean_eval_score: None,
            bad_feedback: 0,
            failed_outcomes: 0,
            tool_loops: 0,
            train: Default::default(),
            validation: Default::default(),
            generalization_gap: None,
            validation_gate: Default::default(),
            evidence: vec![],
        }
    }

    fn candidate(action: CandidateAction) -> GuidanceCandidate {
        GuidanceCandidate {
            id: "c1".into(),
            artifact: row().artifact,
            action,
            status: CandidateStatus::Proposed,
            rationale: "unused".into(),
            evidence: vec![],
            created_at_ms: 0,
            applied_at_ms: None,
            treatment_fingerprint: None,
            experiment_id: None,
            backup_path: None,
        }
    }
}
