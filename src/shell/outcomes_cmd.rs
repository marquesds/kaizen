// SPDX-License-Identifier: AGPL-3.0-or-later
//! `outcomes show` / `outcomes measure` (measure is for ingest; shows in `kaizen help` as hidden if wired).

use crate::collect::outcomes;
use crate::core::config;
use crate::store::{SessionOutcomeRow, Store};
use anyhow::{Context, Result};
use std::path::Path;
use std::time::Duration;

/// Internal worker: run configured commands and write `session_outcomes`.
pub fn cmd_outcomes_measure(workspace: &Path, session_id: &str) -> Result<()> {
    let cfg = config::load(workspace)?;
    let out = &cfg.collect.outcomes;
    if !out.enabled {
        return Ok(());
    }
    let db = workspace.join(".kaizen/kaizen.db");
    let store = Store::open(&db)?;
    let Some(session) = store.get_session(session_id).context("session not found")? else {
        anyhow::bail!("session not in store");
    };
    let root = std::path::PathBuf::from(&session.workspace);
    let tmo = Duration::from_secs(out.timeout_secs.max(1));
    let m = outcomes::run_outcome_measure(&root, &out.test_cmd, out.lint_cmd.as_deref(), tmo);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let row = SessionOutcomeRow {
        session_id: session_id.to_string(),
        test_passed: m.test_passed.map(|v| v as i64),
        test_failed: m.test_failed.map(|v| v as i64),
        test_skipped: m.test_skipped.map(|v| v as i64),
        build_ok: None,
        lint_errors: m.lint_errors.map(|v| v as i64),
        revert_lines_14d: None,
        pr_open: None,
        ci_ok: None,
        measured_at_ms: now,
        measure_error: m.measure_error,
    };
    store.upsert_session_outcome(&row)?;
    Ok(())
}

/// Show one outcome row.
pub fn cmd_outcomes_show(id: &str, workspace: Option<&Path>) -> Result<()> {
    let ws = workspace
        .map(std::path::PathBuf::from)
        .unwrap_or(std::env::current_dir()?);
    let store = Store::open(&ws.join(".kaizen/kaizen.db"))?;
    let r = store
        .get_session_outcome(id)?
        .context("no outcome for session")?;
    print_row(&r);
    Ok(())
}

fn print_row(r: &SessionOutcomeRow) {
    let j = serde_json::json!({
        "session_id": r.session_id,
        "test_passed": r.test_passed,
        "test_failed": r.test_failed,
        "test_skipped": r.test_skipped,
        "lint_errors": r.lint_errors,
        "measured_at_ms": r.measured_at_ms,
        "measure_error": r.measure_error,
    });
    println!("{}", serde_json::to_string_pretty(&j).unwrap_or_default());
}
