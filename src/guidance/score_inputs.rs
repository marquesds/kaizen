// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::core::event::{Event, EventKind};
use crate::feedback::types::FeedbackLabel;
use crate::guidance::inventory;
use crate::guidance::{Artifact, ArtifactKind, ArtifactRef};
use crate::store::{SessionOutcomeRow, Store};
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::path::Path;

pub(crate) struct ScoreInputs {
    pub artifacts: Vec<Artifact>,
    pub sessions: HashMap<ArtifactRef, HashSet<String>>,
    pub costs: HashMap<String, i64>,
    pub evals: HashMap<String, Vec<f64>>,
    pub feedback_bad: HashSet<String>,
    pub outcomes: HashMap<String, SessionOutcomeRow>,
    pub loops: HashMap<String, u64>,
    pub started_at_ms: HashMap<String, u64>,
}

impl ScoreInputs {
    pub fn load(store: &Store, root: &Path, key: &str, start_ms: u64, end_ms: u64) -> Result<Self> {
        let profile = event_profile(store, key, start_ms, end_ms)?;
        Ok(Self {
            artifacts: inventory::scan(root)?,
            sessions: artifact_sessions(store, key, start_ms, end_ms)?,
            costs: store.session_costs_usd_e6_in_window(key, start_ms, end_ms)?,
            evals: evals_by_session(store, start_ms, end_ms)?,
            feedback_bad: bad_feedback(store, start_ms, end_ms)?,
            outcomes: outcomes_by_session(store, key, start_ms, end_ms)?,
            loops: profile.loops,
            started_at_ms: profile.started_at_ms,
        })
    }
}

#[derive(Default)]
struct EventProfile {
    loops: HashMap<String, u64>,
    started_at_ms: HashMap<String, u64>,
}

fn artifact_sessions(
    store: &Store,
    ws: &str,
    start_ms: u64,
    end_ms: u64,
) -> Result<HashMap<ArtifactRef, HashSet<String>>> {
    let mut map = HashMap::new();
    push_sessions(
        &mut map,
        ArtifactKind::Skill,
        store.skills_used_in_window(ws, start_ms, end_ms)?,
    );
    push_sessions(
        &mut map,
        ArtifactKind::Rule,
        store.rules_used_in_window(ws, start_ms, end_ms)?,
    );
    Ok(map)
}

fn push_sessions(
    map: &mut HashMap<ArtifactRef, HashSet<String>>,
    kind: ArtifactKind,
    rows: Vec<(String, String)>,
) {
    for (sid, slug) in rows {
        map.entry(ArtifactRef { kind, slug })
            .or_default()
            .insert(sid);
    }
}

fn evals_by_session(
    store: &Store,
    start_ms: u64,
    end_ms: u64,
) -> Result<HashMap<String, Vec<f64>>> {
    let mut map: HashMap<String, Vec<f64>> = HashMap::new();
    for row in store.list_evals_in_window(start_ms, end_ms)? {
        map.entry(row.session_id).or_default().push(row.score);
    }
    Ok(map)
}

fn bad_feedback(store: &Store, start_ms: u64, end_ms: u64) -> Result<HashSet<String>> {
    Ok(store
        .list_feedback_in_window(start_ms, end_ms)?
        .into_iter()
        .filter(|r| {
            matches!(
                r.label,
                Some(FeedbackLabel::Bad | FeedbackLabel::Regression)
            )
        })
        .map(|r| r.session_id)
        .collect())
}

fn outcomes_by_session(
    store: &Store,
    ws: &str,
    start_ms: u64,
    end_ms: u64,
) -> Result<HashMap<String, SessionOutcomeRow>> {
    Ok(store
        .list_session_outcomes_in_window(ws, start_ms, end_ms)?
        .into_iter()
        .map(|r| (r.session_id.clone(), r))
        .collect())
}

fn event_profile(store: &Store, ws: &str, start_ms: u64, end_ms: u64) -> Result<EventProfile> {
    let mut last: HashMap<String, String> = HashMap::new();
    let mut profile = EventProfile::default();
    for (session, event) in store.retro_events_in_window(ws, start_ms, end_ms)? {
        let id = session.id;
        profile
            .started_at_ms
            .insert(id.clone(), session.started_at_ms);
        if repeated_tool_call(&mut last, &id, event) {
            *profile.loops.entry(id).or_default() += 1;
        }
    }
    Ok(profile)
}

fn repeated_tool_call(last: &mut HashMap<String, String>, sid: &str, event: Event) -> bool {
    event.kind == EventKind::ToolCall
        && event
            .tool
            .is_some_and(|tool| last.insert(sid.into(), tool.clone()).as_ref() == Some(&tool))
}
