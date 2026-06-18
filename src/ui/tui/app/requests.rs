// SPDX-License-Identifier: AGPL-3.0-or-later

use super::*;
use crate::store::SessionFilter;
use anyhow::Result;

impl App {
    pub(in crate::ui::tui) fn refresh_full(&mut self) -> Result<()> {
        self.sessions.reset();
        self.events.clear();
        self.detail_state = DetailState::Idle;
        self.last_session_id = None;
        self.pulse = !self.pulse;
        self.request_session_pages();
        self.sync_metrics_cache();
        self.mark_report_dirty();
        Ok(())
    }

    pub(in crate::ui::tui) fn refresh(&mut self) -> Result<()> {
        self.pulse = !self.pulse;
        self.request_session_pages();
        self.request_selected_detail();
        self.request_event_pages();
        self.sync_metrics_cache();
        self.mark_report_dirty();
        Ok(())
    }

    fn filter(&self) -> SessionFilter {
        SessionFilter {
            agent_prefix: Some(self.agent_filter.trim().to_lowercase())
                .filter(|value| !value.is_empty()),
            status: None,
            since_ms: None,
        }
    }

    pub(in crate::ui::tui) fn request_session_pages(&mut self) {
        let offsets = self
            .sessions
            .needed_page_offsets(self.session_viewport_height);
        offsets.into_iter().for_each(|offset| {
            if self.sessions.request_page(offset) {
                let _ = self.store_tx.send(StoreRequest::SessionsPage {
                    token: self.sessions.generation(),
                    workspace: self.workspace.clone(),
                    offset,
                    limit: self.sessions.page_size,
                    filter: self.filter(),
                });
            }
        });
    }

    pub(super) fn request_feedback_for_viewport(&mut self) {
        let ids = self.feedback_ids();
        if ids.is_empty() {
            return;
        }
        self.feedback_token = self.feedback_token.wrapping_add(1);
        let _ = self.store_tx.send(StoreRequest::Feedback {
            token: self.feedback_token,
            ids,
        });
    }

    fn feedback_ids(&self) -> Vec<String> {
        self.sessions
            .visible_rows(self.session_viewport_height)
            .into_iter()
            .filter_map(|(_, row)| row.map(|session| session.id.clone()))
            .filter(|id| !self.feedback_scores.contains_key(id))
            .collect()
    }

    pub(super) fn request_selected_detail(&mut self) {
        let Some(id) = self.selected_id().map(str::to_string) else {
            self.clear_selection();
            return;
        };
        if self.last_session_id.as_deref() != Some(&id) {
            self.start_detail_request(&id);
        }
        self.request_event_pages();
    }

    fn clear_selection(&mut self) {
        self.detail_state = DetailState::Idle;
        self.events.clear();
        self.last_session_id = None;
    }

    fn start_detail_request(&mut self, id: &str) {
        self.events.reset_for(id);
        self.detail_token = self.detail_token.wrapping_add(1);
        self.detail_state = DetailState::Loading {
            token: self.detail_token,
            session_id: id.to_string(),
        };
        let _ = self.store_tx.send(StoreRequest::Detail {
            token: self.detail_token,
            session_id: id.to_string(),
        });
        self.last_session_id = Some(id.to_string());
    }

    pub(in crate::ui::tui) fn request_event_pages(&mut self) {
        let Some(session_id) = self.events.session_id().map(str::to_string) else {
            return;
        };
        self.events
            .needed_after_seq(self.event_viewport_height)
            .into_iter()
            .for_each(|after_seq| self.request_event_page(&session_id, after_seq));
    }

    fn request_event_page(&mut self, session_id: &str, after_seq: u64) {
        if !self.events.request_page(after_seq) {
            return;
        }
        let _ = self.store_tx.send(StoreRequest::EventsPage {
            token: self.events.generation(),
            session_id: session_id.to_string(),
            after_seq,
            limit: self.events.page_size,
        });
    }

    pub(in crate::ui::tui) fn set_viewport_height(&mut self, height: usize) {
        let viewport = height.saturating_sub(12).max(1);
        self.session_viewport_height = viewport;
        self.event_viewport_height = viewport;
        self.sessions.set_viewport_height(viewport);
        self.events.set_viewport_height(viewport);
    }

    pub(in crate::ui::tui) fn after_session_cursor_move(&mut self) {
        self.request_session_pages();
        self.request_feedback_for_viewport();
        self.request_selected_detail();
    }
}
