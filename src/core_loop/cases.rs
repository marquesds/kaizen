// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::core::event::SessionRecord;
use crate::core_loop::{CaseRecord, CaseRef, CaseStatus};
use crate::store::Store;
use anyhow::{Result, anyhow};
use rusqlite::{OptionalExtension, params};

pub fn create_case(
    store: &Store,
    session: &SessionRecord,
    source_key: &str,
    reason: &str,
    label: Option<String>,
    now_ms: u64,
) -> Result<CaseRecord> {
    let rec = record(session, source_key, reason, label, now_ms);
    store.conn().execute(
        "INSERT OR IGNORE INTO cases
         (id, source_key, session_id, reason, label, status, prompt_fingerprint, metadata_json, created_at_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, 'open', ?6, ?7, ?8)",
        params![rec.id, rec.source_key, rec.session_id, rec.reason, rec.label, rec.prompt_fingerprint, rec.metadata_json, rec.created_at_ms as i64],
    )?;
    get_by_source(store, source_key)
}

pub fn add_ref(store: &Store, case_id: &str, ref_kind: &str, ref_key: &str) -> Result<()> {
    store.conn().execute(
        "INSERT OR IGNORE INTO case_refs (case_id, ref_kind, ref_key) VALUES (?1, ?2, ?3)",
        params![case_id, ref_kind, ref_key],
    )?;
    Ok(())
}

pub fn list(store: &Store, status: Option<CaseStatus>) -> Result<Vec<CaseRecord>> {
    let sql = "SELECT id, source_key, session_id, reason, label, status, prompt_fingerprint, metadata_json, created_at_ms FROM cases WHERE (?1 IS NULL OR status = ?1) ORDER BY created_at_ms DESC";
    let mut stmt = store.conn().prepare(sql)?;
    let rows = stmt.query_map(params![status.map(|s| s.as_str().to_string())], row)?;
    rows.map(|r| r.map_err(anyhow::Error::from)).collect()
}

pub fn get(store: &Store, id: &str) -> Result<CaseRecord> {
    let sql = "SELECT id, source_key, session_id, reason, label, status, prompt_fingerprint, metadata_json, created_at_ms FROM cases WHERE id = ?1";
    store
        .conn()
        .query_row(sql, params![id], row)
        .optional()?
        .ok_or_else(|| anyhow!("case not found: {id}"))
}

pub fn refs(store: &Store, case_id: &str) -> Result<Vec<CaseRef>> {
    let mut stmt = store.conn().prepare("SELECT case_id, ref_kind, ref_key FROM case_refs WHERE case_id = ?1 ORDER BY ref_kind, ref_key")?;
    let rows = stmt.query_map(params![case_id], |r| {
        Ok(CaseRef {
            case_id: r.get(0)?,
            ref_kind: r.get(1)?,
            ref_key: r.get(2)?,
        })
    })?;
    rows.map(|r| r.map_err(anyhow::Error::from)).collect()
}

pub fn archive(store: &Store, id: &str) -> Result<()> {
    store.conn().execute(
        "UPDATE cases SET status = 'archived' WHERE id = ?1",
        params![id],
    )?;
    Ok(())
}

pub fn mine(store: &Store, since_ms: u64, now_ms: u64) -> Result<Vec<CaseRecord>> {
    let mut out = eval_cases(store, since_ms, now_ms)?;
    out.extend(feedback_cases(store, since_ms, now_ms)?);
    Ok(out)
}

fn eval_cases(store: &Store, since_ms: u64, now_ms: u64) -> Result<Vec<CaseRecord>> {
    store
        .list_evals_in_window(since_ms, now_ms)?
        .into_iter()
        .filter(|r| r.flagged || r.score < 0.4)
        .map(|r| {
            from_session(
                store,
                &r.session_id,
                &format!("eval:{}", r.rubric_id),
                "low_eval",
                now_ms,
            )
        })
        .collect()
}

fn feedback_cases(store: &Store, since_ms: u64, now_ms: u64) -> Result<Vec<CaseRecord>> {
    store
        .list_feedback_in_window(since_ms, now_ms)?
        .into_iter()
        .filter(|r| {
            r.label.as_ref().is_some_and(|l| {
                matches!(
                    l,
                    crate::feedback::types::FeedbackLabel::Bad
                        | crate::feedback::types::FeedbackLabel::Regression
                )
            })
        })
        .map(|r| from_session(store, &r.session_id, "feedback:bad", "bad_feedback", now_ms))
        .collect()
}

fn from_session(
    store: &Store,
    id: &str,
    prefix: &str,
    reason: &str,
    now_ms: u64,
) -> Result<CaseRecord> {
    let s = store
        .get_session(id)?
        .ok_or_else(|| anyhow!("session not found: {id}"))?;
    let key = format!("{prefix}:{id}");
    let rec = create_case(store, &s, &key, reason, Some(reason.into()), now_ms)?;
    add_ref(store, &rec.id, "session", id)?;
    Ok(rec)
}

fn record(
    s: &SessionRecord,
    source_key: &str,
    reason: &str,
    label: Option<String>,
    now_ms: u64,
) -> CaseRecord {
    CaseRecord {
        id: uuid::Uuid::now_v7().to_string(),
        source_key: source_key.into(),
        session_id: s.id.clone(),
        reason: reason.into(),
        label,
        status: CaseStatus::Open,
        prompt_fingerprint: s.prompt_fingerprint.clone(),
        metadata_json: "{}".into(),
        created_at_ms: now_ms,
    }
}

fn get_by_source(store: &Store, source_key: &str) -> Result<CaseRecord> {
    let sql = "SELECT id, source_key, session_id, reason, label, status, prompt_fingerprint, metadata_json, created_at_ms FROM cases WHERE source_key = ?1";
    store
        .conn()
        .query_row(sql, params![source_key], row)
        .map_err(Into::into)
}

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<CaseRecord> {
    Ok(CaseRecord {
        id: r.get(0)?,
        source_key: r.get(1)?,
        session_id: r.get(2)?,
        reason: r.get(3)?,
        label: r.get(4)?,
        status: status(r.get::<_, String>(5)?.as_str()),
        prompt_fingerprint: r.get(6)?,
        metadata_json: r.get(7)?,
        created_at_ms: r.get::<_, i64>(8)? as u64,
    })
}

fn status(s: &str) -> CaseStatus {
    if s == "archived" {
        CaseStatus::Archived
    } else {
        CaseStatus::Open
    }
}
