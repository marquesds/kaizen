// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::core::config::EvalConfig;
use crate::core::event::SessionRecord;
use crate::eval::judge::judge_session;
use crate::eval::rubric;
use crate::eval::types::EvalRow;
use crate::store::sqlite::Store;
use anyhow::{Context, Result, bail};
use std::path::Path;

pub fn run_evals(
    store: &Store,
    cfg: &EvalConfig,
    workspace: &Path,
    since_ms: u64,
    dry_run: bool,
) -> Result<Vec<EvalRow>> {
    if !cfg.enabled {
        return Ok(vec![]);
    }
    let rubric =
        rubric::by_id(&cfg.rubric).with_context(|| format!("unknown rubric: {}", cfg.rubric))?;
    let api_key = resolve_api_key(cfg);
    if api_key.is_empty() {
        bail!("eval.api_key not set and ANTHROPIC_API_KEY env var is empty");
    }
    let client = reqwest::blocking::Client::new();
    let candidates = store
        .list_sessions_for_eval(since_ms, cfg.min_cost_usd)
        .context("list sessions for eval")?;
    let results = candidates
        .iter()
        .take(cfg.batch_size)
        .filter_map(|s| eval_one(store, &client, cfg, rubric, s, dry_run))
        .collect();
    let _ = workspace;
    Ok(results)
}

pub fn dry_run_candidates(
    store: &Store,
    cfg: &EvalConfig,
    since_ms: u64,
) -> Result<Vec<SessionRecord>> {
    if !cfg.enabled {
        return Ok(vec![]);
    }
    store
        .list_sessions_for_eval(since_ms, cfg.min_cost_usd)
        .map(|rows| rows.into_iter().take(cfg.batch_size).collect())
}

fn eval_one(
    store: &Store,
    client: &reqwest::blocking::Client,
    cfg: &EvalConfig,
    rubric: &crate::eval::rubric::Rubric,
    session: &SessionRecord,
    dry_run: bool,
) -> Option<EvalRow> {
    if dry_run {
        eprintln!("[dry-run] would eval session {}", session.id);
        return None;
    }
    let events = store.list_events_for_session(&session.id).ok()?;
    let row = judge_session(
        client,
        &cfg.endpoint,
        &resolve_api_key(cfg),
        &cfg.model,
        rubric,
        session,
        &events,
    )
    .ok()?;
    store.upsert_eval(&row).ok()?;
    Some(row)
}

fn resolve_api_key(cfg: &EvalConfig) -> String {
    if !cfg.api_key.is_empty() {
        return cfg.api_key.clone();
    }
    std::env::var("ANTHROPIC_API_KEY").unwrap_or_default()
}
