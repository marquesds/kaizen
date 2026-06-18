// SPDX-License-Identifier: AGPL-3.0-or-later
//! Incremental projector for event-derived rows.

use crate::core::event::Event;
use crate::metrics::types::ToolSpanSample;
use crate::store::tool_span_index::{SpanBuilder, ToolSpanRecord};
use std::collections::{HashMap, HashSet};

mod close;
mod derived;
mod open;
#[cfg(test)]
mod tests;

pub const DEFAULT_ORPHAN_TTL_MS: u64 = 60 * 60 * 1_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenSpan {
    pub(crate) inner: SpanBuilder,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosedSpan {
    pub record: ToolSpanRecord,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectorEvent {
    SpanClosed(Box<ToolSpanRecord>, ToolSpanSample),
    FileTouched { session: String, path: String },
    SkillUsed { session: String, skill: String },
    RuleUsed { session: String, rule: String },
}

#[derive(Debug, Default)]
pub struct Projector {
    open_spans: HashMap<String, HashMap<String, SpanBuilder>>,
    open_order: HashMap<String, Vec<String>>,
    file_touch: HashMap<String, HashSet<String>>,
    skill_use: HashMap<String, HashSet<String>>,
    rule_use: HashMap<String, HashSet<String>>,
    last_seq: HashMap<String, u64>,
}

impl Projector {
    pub fn apply(&mut self, evt: &Event) -> Vec<ProjectorEvent> {
        let mut out = self.apply_derived(evt);
        self.apply_span_event(evt, &mut out);
        self.last_seq.insert(evt.session_id.clone(), evt.seq);
        out
    }

    pub fn flush_session(&mut self, session_id: &str, _now_ms: u64) -> Vec<ProjectorEvent> {
        self.take_session_spans(session_id)
            .into_iter()
            .flat_map(|span| self.close_span(span))
            .collect()
    }

    pub fn flush_expired(&mut self, now_ms: u64, ttl_ms: u64) -> Vec<ProjectorEvent> {
        let ids = self.expired_span_ids(now_ms, ttl_ms);
        self.close_ids(ids)
    }

    pub fn reset_session(&mut self, session_id: &str) {
        self.clear_open(session_id);
        self.file_touch.remove(session_id);
        self.skill_use.remove(session_id);
        self.rule_use.remove(session_id);
        self.last_seq.remove(session_id);
    }

    pub fn last_seq(&self, session_id: &str) -> Option<u64> {
        self.last_seq.get(session_id).copied()
    }
}
