// SPDX-License-Identifier: AGPL-3.0-or-later
//! Scientific skill/rule scoring and candidate commands.

use crate::guidance::{self, ArtifactRef, GuidanceCandidate};
use crate::shell::cli::workspace_path;
use crate::store::Store;
use anyhow::{Result, anyhow};
use std::fmt::Write;
use std::path::Path;

pub fn cmd_score(ws: Option<&Path>, days: u32, min_sessions: u64, json_out: bool) -> Result<()> {
    print!("{}", score_text(ws, days, min_sessions, json_out)?);
    Ok(())
}

pub fn score_text(
    ws: Option<&Path>,
    days: u32,
    min_sessions: u64,
    json_out: bool,
) -> Result<String> {
    let ws = workspace_path(ws)?;
    let store = Store::open(&crate::core::workspace::db_path(&ws)?)?;
    let ws_key = ws.to_string_lossy().to_string();
    let (start, end) = crate::shell::guidance::trailing_window_ms(days);
    let report = guidance::score::build(&store, &ws, &ws_key, start, end, min_sessions)?;
    if json_out {
        return Ok(serde_json::to_string_pretty(&report)?);
    }
    Ok(format_score(&report))
}

pub fn cmd_propose(
    ws: Option<&Path>,
    artifact: &str,
    max_ops: usize,
    llm: bool,
    json_out: bool,
) -> Result<()> {
    print!("{}", propose_text(ws, artifact, max_ops, llm, json_out)?);
    Ok(())
}

pub fn propose_text(
    ws: Option<&Path>,
    artifact: &str,
    max_ops: usize,
    llm: bool,
    json_out: bool,
) -> Result<String> {
    let ws = workspace_path(ws)?;
    let store = Store::open(&crate::core::workspace::db_path(&ws)?)?;
    let artifact =
        ArtifactRef::parse(artifact).ok_or_else(|| anyhow!("use skill:<id> or rule:<id>"))?;
    let candidate = proposed_candidate(&ws, &store, &artifact, max_ops, llm)?;
    store.upsert_guidance_candidate(&candidate)?;
    if json_out {
        Ok(serde_json::to_string_pretty(&candidate)?)
    } else {
        Ok(crate::shell::guidance_candidates::format_candidate(
            &candidate,
        ))
    }
}

fn proposed_candidate(
    ws: &Path,
    store: &Store,
    artifact: &ArtifactRef,
    max_ops: usize,
    llm: bool,
) -> Result<GuidanceCandidate> {
    let row = score_row(ws, store, artifact)?;
    let rejected = store.rejected_guidance_candidates(artifact, 5)?;
    if llm {
        let cfg = crate::core::config::load(ws)?.guidance.proposals;
        return guidance::llm::candidate(&cfg, &row, max_ops, now_ms(), &rejected, ws);
    }
    Ok(guidance::proposals::deterministic(
        &row,
        now_ms(),
        &rejected,
    ))
}

fn score_row(
    ws: &Path,
    store: &Store,
    artifact: &ArtifactRef,
) -> Result<crate::guidance::GuidanceScoreRow> {
    let ws_key = ws.to_string_lossy().to_string();
    let (start, end) = crate::shell::guidance::trailing_window_ms(30);
    let report = guidance::score::build(store, ws, &ws_key, start, end, 30)?;
    report
        .rows
        .into_iter()
        .find(|r| r.artifact == *artifact)
        .ok_or_else(|| anyhow!("artifact not in scorecard: {artifact}"))
}

fn format_score(report: &crate::guidance::GuidanceScoreReport) -> String {
    let mut out = String::new();
    let _ = writeln!(&mut out, "guidance score - {}", report.workspace);
    let _ = writeln!(
        &mut out,
        "{:<8} {:<24} {:>7} {:>7} {:>8} STATE",
        "kind", "id", "score", "val", "sessions"
    );
    for row in &report.rows {
        let _ = writeln!(
            &mut out,
            "{:<8} {:<24} {:>7.1} {:>7.1} {:>8} {:?}",
            row.artifact.kind.as_str(),
            row.artifact.slug,
            row.score,
            row.validation.score,
            row.sessions,
            row.state
        );
    }
    out
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
