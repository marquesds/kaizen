// SPDX-License-Identifier: AGPL-3.0-or-later

use super::*;
use crate::store::SessionPage;

impl App {
    pub(in crate::ui::tui) fn apply_store_response(&mut self, response: StoreResponse) {
        match response {
            StoreResponse::SessionsPage {
                token,
                offset,
                result,
            } => self.apply_session_page(token, offset, result),
            StoreResponse::EventsPage {
                token,
                session_id,
                after_seq,
                result,
            } => self.apply_event_page(token, &session_id, after_seq, result),
            StoreResponse::Detail {
                token,
                session_id,
                result,
            } => self.apply_detail(token, &session_id, result),
            StoreResponse::Feedback { token, result } => self.apply_feedback(token, result),
        }
    }

    fn apply_session_page(
        &mut self,
        token: u64,
        offset: usize,
        result: Result<SessionPage, String>,
    ) {
        if token != self.sessions.generation() {
            return;
        }
        match result {
            Ok(page) => {
                self.sessions.finish_page(offset, page.rows, page.total);
                self.error_note.clear();
                self.request_feedback_for_viewport();
                self.request_selected_detail();
            }
            Err(error) => {
                self.sessions.finish_error(offset);
                self.error_note = error;
            }
        }
    }

    fn apply_event_page(
        &mut self,
        token: u64,
        session_id: &str,
        after_seq: u64,
        result: Result<Vec<Event>, String>,
    ) {
        if token != self.events.generation() || self.events.session_id() != Some(session_id) {
            return;
        }
        match result {
            Ok(rows) => self.events.finish_page(after_seq, rows),
            Err(error) => {
                self.events.finish_error(after_seq);
                self.error_note = error;
            }
        }
    }

    fn apply_detail(&mut self, token: u64, session_id: &str, result: Result<DetailData, String>) {
        if !self.detail_request_matches(token, session_id) {
            return;
        }
        self.detail_state = match result {
            Ok(data) => DetailState::Ready(data),
            Err(error) => DetailState::Error(error),
        };
    }

    fn detail_request_matches(&self, token: u64, session_id: &str) -> bool {
        matches!(
            &self.detail_state,
            DetailState::Loading {
                token: active,
                session_id: active_id,
            } if *active == token && active_id == session_id
        )
    }

    fn apply_feedback(&mut self, token: u64, result: Result<HashMap<String, u8>, String>) {
        if token != self.feedback_token {
            return;
        }
        match result {
            Ok(scores) => self.feedback_scores.extend(scores),
            Err(error) => self.error_note = error,
        }
    }
}
