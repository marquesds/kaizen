// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::guidance::score_inputs::ScoreInputs;
use crate::guidance::score_math::{parts, state, validation_start_ms};
use crate::guidance::types::GuidanceScoreRow;
use crate::guidance::types::{Artifact, ArtifactRef, GuidanceScoreReport};
use crate::store::Store;
use anyhow::Result;
use std::path::Path;

pub fn build(
    store: &Store,
    workspace_root: &Path,
    workspace_key: &str,
    start_ms: u64,
    end_ms: u64,
    min_sessions: u64,
) -> Result<GuidanceScoreReport> {
    let inputs = ScoreInputs::load(store, workspace_root, workspace_key, start_ms, end_ms)?;
    let validation_start_ms = validation_start_ms(start_ms, end_ms);
    let mut rows: Vec<_> = inputs
        .artifacts
        .iter()
        .map(|a| row_for(a, &inputs, min_sessions, validation_start_ms))
        .collect();
    rows.sort_by(|a, b| a.score.total_cmp(&b.score).then(a.path.cmp(&b.path)));
    Ok(GuidanceScoreReport {
        workspace: workspace_key.to_string(),
        window_start_ms: start_ms,
        window_end_ms: end_ms,
        validation_start_ms,
        min_sessions,
        rows,
    })
}

fn row_for(
    a: &Artifact,
    i: &ScoreInputs,
    min_sessions: u64,
    validation_start_ms: u64,
) -> GuidanceScoreRow {
    let ids = i
        .sessions
        .get(&artifact_ref(a))
        .cloned()
        .unwrap_or_default();
    let parts = parts(&ids, i, min_sessions, validation_start_ms);
    GuidanceScoreRow {
        artifact: artifact_ref(a),
        path: a.path.to_string_lossy().to_string(),
        state: state(parts.total.sessions, min_sessions),
        score: parts.total.score,
        sessions: parts.total.sessions,
        avg_cost_usd: parts.total.avg_cost_usd,
        mean_eval_score: parts.total.mean_eval_score,
        bad_feedback: parts.total.bad_feedback,
        failed_outcomes: parts.total.failed_outcomes,
        tool_loops: parts.total.tool_loops,
        train: parts.train,
        validation: parts.validation,
        generalization_gap: parts.generalization_gap,
        validation_gate: parts.validation_gate,
        evidence: parts.evidence,
    }
}

fn artifact_ref(a: &Artifact) -> ArtifactRef {
    ArtifactRef {
        kind: a.kind,
        slug: a.slug.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::guidance::ArtifactState;

    #[test]
    fn unused_skill_is_stale() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let skill = dir.path().join(".cursor/skills/dead");
        std::fs::create_dir_all(&skill)?;
        std::fs::write(skill.join("SKILL.md"), "name: dead\n")?;
        let store = Store::open(&dir.path().join("k.db"))?;
        let report = build(&store, dir.path(), "/ws", 0, 1, 30)?;
        assert_eq!(report.rows[0].artifact.slug, "dead");
        assert_eq!(report.rows[0].state, ArtifactState::Stale);
        Ok(())
    }
}
