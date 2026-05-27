// SPDX-License-Identifier: AGPL-3.0-or-later
//! `kaizen guidance candidates` command helpers.

use crate::guidance::{CandidateAction, CandidateStatus, GuidanceCandidate};
use crate::shell::cli::workspace_path;
use crate::store::Store;
use anyhow::{Result, anyhow};
use std::fmt::Write;
use std::path::Path;

pub enum CandidateOp {
    List { json: bool },
    Show { id: String, json: bool },
    Set { id: String, status: CandidateStatus },
    Validate { id: String },
}

pub fn cmd(ws: Option<&Path>, op: CandidateOp) -> Result<()> {
    print!("{}", text(ws, op)?);
    Ok(())
}

pub fn text(ws: Option<&Path>, op: CandidateOp) -> Result<String> {
    let ws = workspace_path(ws)?;
    let store = Store::open(&crate::core::workspace::db_path(&ws)?)?;
    match op {
        CandidateOp::List { json } => list(&store, json),
        CandidateOp::Show { id, json } => show(&store, &id, json),
        CandidateOp::Set { id, status } => set_status(&store, &id, status),
        CandidateOp::Validate { id } => validate(&store, &ws, &id),
    }
}

fn list(store: &Store, json_out: bool) -> Result<String> {
    let rows = store.list_guidance_candidates()?;
    if json_out {
        return Ok(serde_json::to_string_pretty(&rows)?);
    }
    Ok(rows.into_iter().map(|c| format_candidate(&c)).collect())
}

fn show(store: &Store, id: &str, json_out: bool) -> Result<String> {
    let c = store
        .get_guidance_candidate(id)?
        .ok_or_else(|| anyhow!("candidate not found: {id}"))?;
    if json_out {
        Ok(serde_json::to_string_pretty(&c)?)
    } else {
        Ok(format_candidate(&c))
    }
}

fn set_status(store: &Store, id: &str, status: CandidateStatus) -> Result<String> {
    store.set_guidance_candidate_status(id, status)?;
    Ok(format!("{} {id}\n", status.as_str()))
}

fn validate(store: &Store, ws: &Path, id: &str) -> Result<String> {
    let c = store
        .get_guidance_candidate(id)?
        .ok_or_else(|| anyhow!("candidate not found: {id}"))?;
    let gate = crate::guidance::validation::evaluate(store, ws, &c)?;
    if let Some(status) = gate.next_status() {
        store.set_guidance_candidate_status(id, status)?;
    }
    Ok(format_gate(&gate))
}

fn format_gate(g: &crate::guidance::validation::ValidationGate) -> String {
    let mut out = String::new();
    let _ = writeln!(&mut out, "id:        {}", g.candidate_id);
    let _ = writeln!(&mut out, "experiment: {}", g.experiment_id);
    let _ = writeln!(&mut out, "outcome:   {:?}", g.outcome);
    let _ = writeln!(&mut out, "arms:      {} / {}", g.n_control, g.n_treatment);
    if let Some(delta) = g.delta_pct {
        let _ = writeln!(&mut out, "delta:     {delta:.1}%");
    }
    out
}

pub(crate) fn format_candidate(c: &GuidanceCandidate) -> String {
    let mut out = String::new();
    let _ = writeln!(&mut out, "id:       {}", c.id);
    let _ = writeln!(&mut out, "artifact: {}", c.artifact);
    let _ = writeln!(&mut out, "status:   {}", c.status.as_str());
    let _ = writeln!(&mut out, "action:   {}", action_label(&c.action));
    let _ = writeln!(&mut out, "why:      {}", c.rationale);
    out
}

fn action_label(action: &CandidateAction) -> &'static str {
    match action {
        CandidateAction::Delete => "delete",
        CandidateAction::Replace { .. } => "replace",
        CandidateAction::ReviewOnly => "review_only",
    }
}
