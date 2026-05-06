// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::core::config;
use crate::eval::engine::run_evals;
use crate::store::sqlite::Store;
use anyhow::Result;
use std::path::{Path, PathBuf};

pub fn cmd_eval_run(workspace: Option<&Path>, since_days: u64, dry_run: bool) -> Result<()> {
    let ws = resolve_ws(workspace)?;
    let cfg = config::load(&ws)?;
    let store = open_store(&ws)?;
    let since_ms = since_ms_from_days(since_days);
    let rows = run_evals(&store, &cfg.eval, &ws, since_ms, dry_run)?;
    if dry_run {
        println!("dry-run: {} sessions would be evaluated", rows.len());
    } else {
        println!("evaluated {} session(s)", rows.len());
        for r in &rows {
            println!(
                "  {} score={:.2} flagged={}",
                r.session_id, r.score, r.flagged
            );
        }
    }
    Ok(())
}

pub fn cmd_eval_list(workspace: Option<&Path>, min_score: f64, json: bool) -> Result<()> {
    let ws = resolve_ws(workspace)?;
    let store = open_store(&ws)?;
    let now = now_ms();
    let rows = store.list_evals_in_window(0, now)?;
    let filtered: Vec<_> = rows.iter().filter(|r| r.score >= min_score).collect();
    if json {
        println!("{}", serde_json::to_string_pretty(&filtered)?);
    } else {
        for r in &filtered {
            println!(
                "{}\tscore={:.2}\tflagged={}\t{}",
                r.session_id, r.score, r.flagged, r.rationale
            );
        }
    }
    Ok(())
}

pub fn cmd_eval_prompt(workspace: Option<&Path>, session_id: &str, rubric_id: &str) -> Result<()> {
    let ws = resolve_ws(workspace)?;
    let store = open_store(&ws)?;
    let session = store
        .get_session(session_id)?
        .ok_or_else(|| anyhow::anyhow!("session not found: {session_id}"))?;
    let events = store.list_events_for_session(session_id)?;
    let rubric = crate::eval::rubric::by_id(rubric_id)
        .ok_or_else(|| anyhow::anyhow!("unknown rubric: {rubric_id}"))?;
    println!(
        "{}",
        crate::eval::judge::build_prompt(rubric, &session, &events)
    );
    Ok(())
}

fn resolve_ws(workspace: Option<&Path>) -> Result<PathBuf> {
    crate::core::workspace::resolve(workspace)
}

fn open_store(ws: &Path) -> Result<Store> {
    Store::open(&crate::core::workspace::db_path(ws)?)
}

fn since_ms_from_days(days: u64) -> u64 {
    now_ms().saturating_sub(days * 86_400_000)
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
