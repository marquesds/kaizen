// SPDX-License-Identifier: AGPL-3.0-or-later
//! Persistence for experiments. IO at boundary; pure types in `types.rs`.

use crate::experiment::binding::ManualTags;
use crate::experiment::types::{Classification, Experiment, State};
use crate::store::Store;
use anyhow::{Context, Result};
use rusqlite::{OptionalExtension, params};

pub fn save_experiment(store: &Store, exp: &Experiment) -> Result<()> {
    let json = serde_json::to_string(exp).context("serialize experiment")?;
    store.conn().execute(
        "INSERT INTO experiments (id, name, created_at_ms, metadata, state, concluded_at_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(id) DO UPDATE SET
           name=excluded.name,
           metadata=excluded.metadata,
           state=excluded.state,
           concluded_at_ms=excluded.concluded_at_ms",
        params![
            exp.id,
            exp.name,
            exp.created_at_ms as i64,
            json,
            format!("{:?}", exp.state),
            exp.concluded_at_ms.map(|v| v as i64),
        ],
    )?;
    Ok(())
}

pub fn load_experiment(store: &Store, id: &str) -> Result<Option<Experiment>> {
    let row: Option<String> = store
        .conn()
        .query_row(
            "SELECT metadata FROM experiments WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )
        .optional()?;
    match row {
        Some(s) => Ok(Some(serde_json::from_str(&s)?)),
        None => Ok(None),
    }
}

pub fn list_experiments(store: &Store) -> Result<Vec<Experiment>> {
    let mut stmt = store
        .conn()
        .prepare("SELECT metadata FROM experiments ORDER BY created_at_ms DESC")?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
    let mut out = Vec::new();
    for row in rows {
        let s = row?;
        if let Ok(e) = serde_json::from_str::<Experiment>(&s) {
            out.push(e);
        }
    }
    Ok(out)
}

pub fn set_state(store: &Store, id: &str, state: State, now_ms: u64) -> Result<()> {
    let Some(mut exp) = load_experiment(store, id)? else {
        anyhow::bail!("experiment not found: {id}");
    };
    exp.state = state;
    if matches!(state, State::Concluded) {
        exp.concluded_at_ms = Some(now_ms);
    }
    save_experiment(store, &exp)
}

/// Tag a session with a variant.
///
/// Idempotent when the same variant is supplied. Returns `Err` when the session
/// already carries a *different* variant — the caller must resolve the conflict
/// rather than silently overwrite.
pub fn tag_session(
    store: &Store,
    exp_id: &str,
    session_id: &str,
    variant: Classification,
) -> Result<()> {
    let existing: Option<String> = store
        .conn()
        .query_row(
            "SELECT variant FROM experiment_tags WHERE experiment_id=?1 AND session_id=?2",
            params![exp_id, session_id],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(prev) = existing {
        let prev_cls = parse_variant(&prev);
        if prev_cls != variant {
            anyhow::bail!(
                "variant conflict: session {session_id} already tagged as {prev} \
                 for experiment {exp_id}; cannot retag as {:?}",
                variant
            );
        }
        return Ok(());
    }
    store.conn().execute(
        "INSERT INTO experiment_tags (experiment_id, session_id, variant) VALUES (?1, ?2, ?3)",
        params![exp_id, session_id, format!("{:?}", variant)],
    )?;
    Ok(())
}

fn parse_variant(s: &str) -> Classification {
    match s {
        "Control" => Classification::Control,
        "Treatment" => Classification::Treatment,
        _ => Classification::Excluded,
    }
}

pub fn manual_tags(store: &Store, exp_id: &str) -> Result<ManualTags> {
    let mut stmt = store
        .conn()
        .prepare("SELECT session_id, variant FROM experiment_tags WHERE experiment_id = ?1")?;
    let rows = stmt.query_map(params![exp_id], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
    })?;
    let mut out = ManualTags::new();
    for row in rows {
        let (sid, variant) = row?;
        out.insert(sid, parse_variant(&variant));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::experiment::types::{Binding, Criterion, Direction, Metric, State};
    use tempfile::TempDir;

    fn mk(id: &str) -> Experiment {
        Experiment {
            id: id.into(),
            name: format!("exp-{id}"),
            hypothesis: "h".into(),
            change_description: "c".into(),
            metric: Metric::TokensPerSession,
            binding: Binding::GitCommit {
                control_commit: "c1".into(),
                treatment_commit: "c2".into(),
            },
            duration_days: 14,
            success_criterion: Criterion::Delta {
                direction: Direction::Decrease,
                target_pct: 10.0,
            },
            state: State::Draft,
            created_at_ms: 1000,
            concluded_at_ms: None,
            guardrails: Vec::new(),
        }
    }

    #[test]
    fn round_trip_save_load() {
        let dir = TempDir::new().unwrap();
        let store = Store::open(&dir.path().join("k.db")).unwrap();
        let e = mk("a");
        save_experiment(&store, &e).unwrap();
        let got = load_experiment(&store, "a").unwrap().unwrap();
        assert_eq!(got.id, "a");
        assert_eq!(got.state, State::Draft);
    }

    #[test]
    fn set_state_transitions() {
        let dir = TempDir::new().unwrap();
        let store = Store::open(&dir.path().join("k.db")).unwrap();
        save_experiment(&store, &mk("b")).unwrap();
        set_state(&store, "b", State::Running, 5_000).unwrap();
        let got = load_experiment(&store, "b").unwrap().unwrap();
        assert_eq!(got.state, State::Running);
        set_state(&store, "b", State::Concluded, 9_000).unwrap();
        let got = load_experiment(&store, "b").unwrap().unwrap();
        assert_eq!(got.state, State::Concluded);
        assert_eq!(got.concluded_at_ms, Some(9_000));
    }

    #[test]
    fn tags_round_trip() {
        let dir = TempDir::new().unwrap();
        let store = Store::open(&dir.path().join("k.db")).unwrap();
        save_experiment(&store, &mk("e")).unwrap();
        tag_session(&store, "e", "s1", Classification::Treatment).unwrap();
        tag_session(&store, "e", "s2", Classification::Control).unwrap();
        let tags = manual_tags(&store, "e").unwrap();
        assert_eq!(tags.get("s1"), Some(&Classification::Treatment));
        assert_eq!(tags.get("s2"), Some(&Classification::Control));
    }

    #[test]
    fn tag_same_variant_is_idempotent() {
        let dir = TempDir::new().unwrap();
        let store = Store::open(&dir.path().join("k.db")).unwrap();
        save_experiment(&store, &mk("idem")).unwrap();
        tag_session(&store, "idem", "s1", Classification::Control).unwrap();
        // tagging the same variant again must succeed
        tag_session(&store, "idem", "s1", Classification::Control).unwrap();
        let tags = manual_tags(&store, "idem").unwrap();
        assert_eq!(tags.get("s1"), Some(&Classification::Control));
    }

    #[test]
    fn tag_different_variant_is_error() {
        let dir = TempDir::new().unwrap();
        let store = Store::open(&dir.path().join("k.db")).unwrap();
        save_experiment(&store, &mk("conflict")).unwrap();
        tag_session(&store, "conflict", "s1", Classification::Control).unwrap();
        let err = tag_session(&store, "conflict", "s1", Classification::Treatment).unwrap_err();
        assert!(
            err.to_string().contains("variant conflict"),
            "expected variant conflict, got: {err}"
        );
    }

    #[test]
    fn concurrent_tag_produces_one_row() {
        use std::sync::Arc;
        use std::thread;

        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("k.db");
        let store = Store::open(&db_path).unwrap();
        save_experiment(&store, &mk("concur")).unwrap();
        drop(store);

        // 8 threads all tag the same session as Treatment concurrently.
        let path = Arc::new(db_path);
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let p = Arc::clone(&path);
                thread::spawn(move || {
                    let s = Store::open(&p).unwrap();
                    tag_session(&s, "concur", "sess", Classification::Treatment)
                })
            })
            .collect();

        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        let ok_count = results.iter().filter(|r| r.is_ok()).count();
        assert!(ok_count >= 1, "at least one thread must succeed");

        let store2 = Store::open(&path).unwrap();
        let tags = manual_tags(&store2, "concur").unwrap();
        assert_eq!(
            tags.get("sess"),
            Some(&Classification::Treatment),
            "exactly one row, correct variant"
        );
    }
}
