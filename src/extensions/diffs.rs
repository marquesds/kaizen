// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::store::Store;
use anyhow::Result;
use rusqlite::params;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StepDiff {
    pub session_id: String,
    pub span_id: String,
    pub files: Vec<String>,
    pub added_lines: u64,
    pub removed_lines: u64,
    pub raw_patch_stored: bool,
}

pub fn refresh_session(
    store: &Store,
    session_id: &str,
    capture_raw_patch: bool,
) -> Result<Vec<StepDiff>> {
    let rows: Vec<StepDiff> = store
        .tool_spans_for_session(session_id)?
        .into_iter()
        .filter(|span| !span.paths.is_empty())
        .map(|span| StepDiff {
            session_id: session_id.to_string(),
            span_id: span.span_id,
            files: span.paths,
            added_lines: 0,
            removed_lines: 0,
            raw_patch_stored: capture_raw_patch,
        })
        .collect();
    store
        .conn()
        .execute("DELETE FROM step_diffs WHERE session_id = ?1", [session_id])?;
    rows.iter().try_for_each(|row| insert(store, row))?;
    Ok(rows)
}

fn insert(store: &Store, row: &StepDiff) -> Result<()> {
    let files_json = serde_json::to_string(&row.files)?;
    store.conn().execute(
        "INSERT OR REPLACE INTO step_diffs(
            session_id, span_id, files_json, added_lines, removed_lines, raw_patch
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            row.session_id,
            row.span_id,
            files_json,
            row.added_lines as i64,
            row.removed_lines as i64,
            Option::<String>::None,
        ],
    )?;
    Ok(())
}
