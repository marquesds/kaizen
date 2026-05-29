// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::core::event::Event;
use crate::store::Store;
use anyhow::Result;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

const GENESIS: &str = "blake3:genesis";

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct HashChainReport {
    pub checked_events: u64,
    pub verified_events: u64,
    pub unverifiable_events: u64,
    pub broken_events: Vec<HashBreak>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct HashBreak {
    pub session_id: String,
    pub seq: u64,
    pub reason: String,
}

pub fn store_event_hash(store: &Store, event: &Event) -> Result<()> {
    let prev = previous_hash(store, &event.session_id, event.seq)?;
    let hash = event_hash(&prev, event)?;
    store.conn().execute(
        "INSERT INTO event_hashes(session_id, seq, prev_hash, event_hash)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(session_id, seq) DO UPDATE SET
            prev_hash=excluded.prev_hash, event_hash=excluded.event_hash",
        params![event.session_id, event.seq as i64, prev, hash],
    )?;
    Ok(())
}

pub fn verify(store: &Store, workspace: &str, session_id: Option<&str>) -> Result<HashChainReport> {
    session_ids(store, workspace, session_id)?.iter().try_fold(
        HashChainReport::default(),
        |mut report, id| {
            verify_session(store, id, &mut report)?;
            Ok(report)
        },
    )
}

fn verify_session(store: &Store, session_id: &str, report: &mut HashChainReport) -> Result<()> {
    let mut prev = None;
    for event in store.list_events_for_session(session_id)? {
        report.checked_events += 1;
        match stored_hash(store, session_id, event.seq)? {
            Some(row) => verify_row(report, &mut prev, &event, row)?,
            None => report.unverifiable_events += 1,
        }
    }
    Ok(())
}

fn verify_row(
    report: &mut HashChainReport,
    prev: &mut Option<String>,
    event: &Event,
    row: (String, String),
) -> Result<()> {
    let (stored_prev, stored_hash) = row;
    if prev.as_deref().is_some_and(|p| p != stored_prev) {
        push_break(report, event, "previous hash mismatch");
    }
    if event_hash(&stored_prev, event)? != stored_hash {
        push_break(report, event, "event hash mismatch");
    } else {
        report.verified_events += 1;
    }
    *prev = Some(stored_hash);
    Ok(())
}

fn push_break(report: &mut HashChainReport, event: &Event, reason: &str) {
    report.broken_events.push(HashBreak {
        session_id: event.session_id.clone(),
        seq: event.seq,
        reason: reason.to_string(),
    });
}

fn session_ids(store: &Store, workspace: &str, session_id: Option<&str>) -> Result<Vec<String>> {
    Ok(match session_id {
        Some(id) => vec![id.to_string()],
        None => store
            .list_sessions(workspace)?
            .into_iter()
            .map(|session| session.id)
            .collect(),
    })
}

fn stored_hash(store: &Store, session_id: &str, seq: u64) -> Result<Option<(String, String)>> {
    store
        .conn()
        .query_row(
            "SELECT prev_hash, event_hash FROM event_hashes WHERE session_id=?1 AND seq=?2",
            params![session_id, seq as i64],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(Into::into)
}

fn previous_hash(store: &Store, session_id: &str, seq: u64) -> Result<String> {
    store
        .conn()
        .query_row(
            "SELECT event_hash FROM event_hashes
             WHERE session_id=?1 AND seq < ?2 ORDER BY seq DESC LIMIT 1",
            params![session_id, seq as i64],
            |row| row.get(0),
        )
        .optional()
        .map(|v| v.unwrap_or_else(|| GENESIS.to_string()))
        .map_err(Into::into)
}

fn event_hash(prev: &str, event: &Event) -> Result<String> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(prev.as_bytes());
    hasher.update(&serde_json::to_vec(event)?);
    Ok(format!(
        "blake3:{}",
        hex::encode(hasher.finalize().as_bytes())
    ))
}
