// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::feedback::types::{FeedbackLabel, FeedbackRecord, FeedbackScore};
use crate::store::Store;
use anyhow::Result;
use std::path::Path;

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn parse_since(since: &str) -> Option<u64> {
    let s = since.trim();
    let (n, unit) = s.split_at(s.len().saturating_sub(1));
    let days: u64 = match unit {
        "d" => n.parse().ok()?,
        "w" => n.parse::<u64>().ok()?.checked_mul(7)?,
        _ => return None,
    };
    Some(days * 86_400_000)
}

fn open_store(workspace: Option<&Path>) -> Result<Store> {
    let ws = crate::core::workspace::resolve(workspace)?;
    Store::open(&crate::core::workspace::db_path(&ws))
}

/// `kaizen sessions annotate <id>` — attach score/label/note to a session.
pub fn cmd_sessions_annotate(
    id: &str,
    score: Option<u8>,
    label: Option<FeedbackLabel>,
    note: Option<String>,
    workspace: Option<&Path>,
) -> Result<()> {
    let store = open_store(workspace)?;
    let record = FeedbackRecord {
        id: uuid::Uuid::now_v7().to_string(),
        session_id: id.to_string(),
        score: score.and_then(FeedbackScore::new),
        label,
        note,
        created_at_ms: now_ms(),
    };
    store.upsert_feedback(&record)?;
    println!("annotated session {id}");
    Ok(())
}

/// `kaizen feedback list` — show feedback records in a time window.
pub fn cmd_feedback_list(
    workspace: Option<&Path>,
    label_filter: Option<FeedbackLabel>,
    since: Option<String>,
    json: bool,
) -> Result<()> {
    let store = open_store(workspace)?;
    let now = now_ms();
    let start_ms = since
        .as_deref()
        .and_then(parse_since)
        .map(|d| now.saturating_sub(d))
        .unwrap_or(0);
    let mut records = store.list_feedback_in_window(start_ms, now)?;
    if let Some(lf) = &label_filter {
        records.retain(|r| r.label.as_ref() == Some(lf));
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&records)?);
        return Ok(());
    }
    if records.is_empty() {
        println!("no feedback records");
        return Ok(());
    }
    for r in &records {
        let score = r
            .score
            .as_ref()
            .map(|s| s.0.to_string())
            .unwrap_or_else(|| "-".into());
        let label = r
            .label
            .as_ref()
            .map(|l| l.to_string())
            .unwrap_or_else(|| "-".into());
        let note = r.note.as_deref().unwrap_or("-");
        println!(
            "{} session={} score={} label={} note={}",
            r.id, r.session_id, score, label, note
        );
    }
    Ok(())
}
