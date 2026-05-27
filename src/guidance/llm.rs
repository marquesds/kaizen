// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::guidance::{self, CandidateAction, GuidanceCandidate, GuidanceScoreRow};
use anyhow::{Result, anyhow, bail};
use serde::Deserialize;
use serde_json::json;
use std::path::Path;

pub fn candidate(
    cfg: &crate::core::config::GuidanceProposalConfig,
    row: &GuidanceScoreRow,
    max_ops: usize,
    now_ms: u64,
    rejected: &[GuidanceCandidate],
    workspace: &Path,
) -> Result<GuidanceCandidate> {
    if !cfg.enabled {
        bail!("guidance.proposals.enabled is false; omit --llm or enable it");
    }
    let text = call(cfg, row, max_ops.min(cfg.max_ops), rejected, workspace)?;
    let resp: LlmProposal = serde_json::from_str(&text)?;
    let mut candidate = guidance::proposals::deterministic(row, now_ms, rejected);
    candidate.action = resp.action.into_action();
    candidate.rationale = resp.rationale;
    Ok(candidate)
}

#[derive(Deserialize)]
struct LlmProposal {
    action: LlmAction,
    rationale: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case", tag = "op")]
enum LlmAction {
    Delete,
    Replace { content: String },
    ReviewOnly,
}

impl LlmAction {
    fn into_action(self) -> CandidateAction {
        match self {
            Self::Delete => CandidateAction::Delete,
            Self::Replace { content } => CandidateAction::Replace { content },
            Self::ReviewOnly => CandidateAction::ReviewOnly,
        }
    }
}

fn call(
    cfg: &crate::core::config::GuidanceProposalConfig,
    row: &GuidanceScoreRow,
    max_ops: usize,
    rejected: &[GuidanceCandidate],
    workspace: &Path,
) -> Result<String> {
    let key = api_key(cfg);
    if key.is_empty() {
        bail!("guidance proposal api_key or ANTHROPIC_API_KEY required");
    }
    let url = format!("{}/v1/messages", cfg.endpoint.trim_end_matches('/'));
    let raw: serde_json::Value = reqwest::blocking::Client::new()
        .post(url)
        .header("x-api-key", key)
        .header("anthropic-version", "2023-06-01")
        .json(&body(cfg, row, max_ops, rejected, workspace))
        .send()?
        .json()?;
    raw["content"][0]["text"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| anyhow!("missing proposal text"))
}

fn body(
    cfg: &crate::core::config::GuidanceProposalConfig,
    row: &GuidanceScoreRow,
    max_ops: usize,
    rejected: &[GuidanceCandidate],
    workspace: &Path,
) -> serde_json::Value {
    json!({
        "model": cfg.model,
        "max_tokens": 2048,
        "messages": [{"role": "user", "content": proposal_prompt(cfg, row, max_ops, rejected, workspace)}],
    })
}

fn proposal_prompt(
    cfg: &crate::core::config::GuidanceProposalConfig,
    row: &GuidanceScoreRow,
    max_ops: usize,
    rejected: &[GuidanceCandidate],
    workspace: &Path,
) -> String {
    let prompt = format!(
        "Propose at most {max_ops} safe edit op for {}. Evidence: {:?}. Reply JSON only: {{\"action\":{{\"op\":\"review_only\"}},\"rationale\":\"...\"}} or delete/replace.",
        row.artifact,
        evidence_with_memory(row, rejected)
    );
    redact_prompt(cfg, &prompt, workspace)
}

fn evidence_with_memory(row: &GuidanceScoreRow, rejected: &[GuidanceCandidate]) -> Vec<String> {
    row.evidence
        .iter()
        .cloned()
        .chain(guidance::proposals::rejected_memory_lines(rejected))
        .collect()
}

fn redact_prompt(
    cfg: &crate::core::config::GuidanceProposalConfig,
    prompt: &str,
    workspace: &Path,
) -> String {
    if cfg.redact {
        crate::sync::redact::redact_string(prompt, workspace, &[0u8; 32])
    } else {
        prompt.to_string()
    }
}

fn api_key(cfg: &crate::core::config::GuidanceProposalConfig) -> String {
    if cfg.api_key.is_empty() {
        std::env::var("ANTHROPIC_API_KEY").unwrap_or_default()
    } else {
        cfg.api_key.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::guidance::{ArtifactKind, ArtifactRef, ArtifactState, CandidateStatus};

    #[test]
    fn prompt_includes_rejected_memory_without_content() {
        let cfg = crate::core::config::GuidanceProposalConfig::default();
        let prompt = proposal_prompt(&cfg, &row(), 3, &[candidate()], Path::new("/ws"));
        assert!(prompt.contains("replace"));
        assert!(prompt.contains("bad edit"));
        assert!(!prompt.contains("raw replacement"));
    }

    #[test]
    fn prompt_redacts_when_enabled() {
        let cfg = crate::core::config::GuidanceProposalConfig {
            redact: true,
            ..Default::default()
        };
        let mut row = row();
        row.evidence = vec!["email a@example.com path /Users/me/secret.txt".into()];
        let prompt = proposal_prompt(&cfg, &row, 3, &[], Path::new("/ws"));
        assert!(!prompt.contains("a@example.com"));
        assert!(!prompt.contains("/Users/me"));
    }

    fn row() -> GuidanceScoreRow {
        GuidanceScoreRow {
            artifact: ArtifactRef {
                kind: ArtifactKind::Skill,
                slug: "tdd".into(),
            },
            path: "tdd".into(),
            state: ArtifactState::Current,
            score: 50.0,
            sessions: 1,
            avg_cost_usd: None,
            mean_eval_score: None,
            bad_feedback: 0,
            failed_outcomes: 0,
            tool_loops: 0,
            train: Default::default(),
            validation: Default::default(),
            generalization_gap: None,
            validation_gate: Default::default(),
            evidence: vec!["low score".into()],
        }
    }

    fn candidate() -> GuidanceCandidate {
        GuidanceCandidate {
            id: "r1".into(),
            artifact: row().artifact,
            action: CandidateAction::Replace {
                content: "raw replacement".into(),
            },
            status: CandidateStatus::Rejected,
            rationale: "bad edit".into(),
            evidence: vec![],
            created_at_ms: 0,
            applied_at_ms: None,
            treatment_fingerprint: None,
            experiment_id: None,
            backup_path: None,
        }
    }
}
