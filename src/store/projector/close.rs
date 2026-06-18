// SPDX-License-Identifier: AGPL-3.0-or-later

use super::{Projector, ProjectorEvent};
use crate::metrics::types::ToolSpanSample;
use crate::store::projector_hierarchy::{add_child, detach_chain, seed_subtree};
use crate::store::tool_span_index::{SpanBuilder, ToolSpanRecord, span_start};

impl Projector {
    pub(super) fn take_session_spans(&mut self, session_id: &str) -> Vec<SpanBuilder> {
        let mut ids = self.open_order.remove(session_id).unwrap_or_default();
        ids.reverse();
        let Some(mut spans) = self.open_spans.remove(session_id) else {
            return Vec::new();
        };
        ids.into_iter().filter_map(|id| spans.remove(&id)).collect()
    }

    pub(super) fn expired_span_ids(&self, now_ms: u64, ttl_ms: u64) -> Vec<(String, String)> {
        self.open_spans
            .iter()
            .flat_map(|(session, spans)| {
                spans.iter().filter_map(move |(id, span)| {
                    let started = span_start(span)?;
                    (now_ms.saturating_sub(started) > ttl_ms).then(|| (session.clone(), id.clone()))
                })
            })
            .collect()
    }

    pub(super) fn close_ids(&mut self, ids: Vec<(String, String)>) -> Vec<ProjectorEvent> {
        ids.into_iter().fold(Vec::new(), |mut out, (session, id)| {
            if let Some(span) = self.remove_open(&session, &id) {
                out.extend(self.close_span(span));
            }
            out
        })
    }

    pub(super) fn clear_open(&mut self, session_id: &str) {
        self.open_order.remove(session_id);
        self.open_spans.remove(session_id);
    }

    pub(super) fn close_span(&mut self, mut span: SpanBuilder) -> Vec<ProjectorEvent> {
        seed_subtree(&mut span);
        self.propagate_to_parent(&span);
        let record = ToolSpanRecord::from_builder(&span);
        let sample = ToolSpanSample::from(&record);
        vec![ProjectorEvent::SpanClosed(Box::new(record), sample)]
    }

    fn propagate_to_parent(&mut self, span: &SpanBuilder) {
        let Some(parent_id) = span.parent_span_id.as_ref() else {
            return;
        };
        if let Some(parent) = self
            .open_spans
            .get_mut(&span.session_id)
            .and_then(|spans| spans.get_mut(parent_id))
        {
            add_child(parent, span);
        }
    }

    pub(super) fn remove_open(&mut self, session_id: &str, span_id: &str) -> Option<SpanBuilder> {
        let spans = self.open_spans.get_mut(session_id)?;
        let span = spans.remove(span_id)?;
        if let Some(order) = self.open_order.get_mut(session_id) {
            detach_chain(order, spans, &span);
        }
        self.prune_empty_session(session_id);
        Some(span)
    }

    fn prune_empty_session(&mut self, session_id: &str) {
        if self
            .open_spans
            .get(session_id)
            .is_some_and(|spans| spans.is_empty())
        {
            self.open_spans.remove(session_id);
        }
        if self.open_order.get(session_id).is_some_and(Vec::is_empty) {
            self.open_order.remove(session_id);
        }
    }
}
