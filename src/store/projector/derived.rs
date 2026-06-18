// SPDX-License-Identifier: AGPL-3.0-or-later

use super::{Projector, ProjectorEvent};
use crate::core::event::Event;
use crate::store::event_index::{
    paths_from_event_payload, rules_from_event_json, skills_from_event_json,
};
use std::collections::{HashMap, HashSet};

impl Projector {
    pub(super) fn apply_derived(&mut self, event: &Event) -> Vec<ProjectorEvent> {
        let session = &event.session_id;
        let files = fresh(
            &mut self.file_touch,
            session,
            paths_from_event_payload(&event.payload),
        );
        let skills = fresh(
            &mut self.skill_use,
            session,
            skills_from_event_json(&event.payload),
        );
        let rules = fresh(
            &mut self.rule_use,
            session,
            rules_from_event_json(&event.payload),
        );
        files
            .into_iter()
            .map(|path| ProjectorEvent::FileTouched {
                session: session.clone(),
                path,
            })
            .chain(skills.into_iter().map(|skill| ProjectorEvent::SkillUsed {
                session: session.clone(),
                skill,
            }))
            .chain(rules.into_iter().map(|rule| ProjectorEvent::RuleUsed {
                session: session.clone(),
                rule,
            }))
            .collect()
    }
}

fn fresh(
    cache: &mut HashMap<String, HashSet<String>>,
    session: &str,
    values: impl IntoIterator<Item = String>,
) -> Vec<String> {
    let seen = cache.entry(session.to_owned()).or_default();
    values
        .into_iter()
        .filter(|value| seen.insert(value.clone()))
        .collect()
}
