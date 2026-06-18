// SPDX-License-Identifier: AGPL-3.0-or-later

mod open;
mod requests;
mod responses;

use super::view::{DetailData, DetailState, EventView, SessionView};
use super::worker::{StoreRequest, StoreResponse};
use crate::core::event::{Event, SessionRecord};
use crate::metrics::types::MetricsReport;
use crate::visualization::VisualizationReport;
use arc_swap::ArcSwapOption;
use std::collections::HashMap;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tokio::sync::{Notify, mpsc};

pub(super) const DEFAULT_VIEWPORT_HEIGHT: usize = 32;

pub(super) struct App {
    pub(super) sessions: SessionView,
    pub(super) events: EventView,
    pub(super) detail_state: DetailState,
    pub(super) agent_filter: String,
    pub(super) filter_mode: bool,
    pub(super) filter_buf: String,
    pub(super) clipboard_note: String,
    pub(super) left_focus: bool,
    pub(super) show_help: bool,
    pub(super) detail: bool,
    pub(super) show_metrics: bool,
    pub(super) metrics: Option<MetricsReport>,
    pub(super) visualization: Option<VisualizationReport>,
    pub(super) metrics_cache: Arc<ArcSwapOption<MetricsReport>>,
    pub(super) visualization_cache: Arc<ArcSwapOption<VisualizationReport>>,
    pub(super) report_dirty: Arc<AtomicBool>,
    pub(super) report_notify: Arc<Notify>,
    pub(super) pulse: bool,
    pub(super) workspace: String,
    pub(super) store_tx: mpsc::UnboundedSender<StoreRequest>,
    pub(super) store_rx: mpsc::UnboundedReceiver<StoreResponse>,
    pub(super) feedback_scores: HashMap<String, u8>,
    pub(super) feedback_token: u64,
    pub(super) detail_token: u64,
    pub(super) last_session_id: Option<String>,
    pub(super) session_viewport_height: usize,
    pub(super) event_viewport_height: usize,
    pub(super) error_note: String,
}

impl App {
    pub(super) fn sync_metrics_cache(&mut self) {
        self.metrics = self.metrics_cache.load_full().as_deref().cloned();
        self.visualization = self.visualization_cache.load_full().as_deref().cloned();
    }

    pub(super) fn mark_report_dirty(&self) {
        self.report_dirty.store(true, Ordering::Release);
        self.report_notify.notify_one();
    }

    pub(super) fn selected_session(&self) -> Option<&SessionRecord> {
        self.sessions.selected()
    }

    pub(super) fn selected_id(&self) -> Option<&str> {
        self.selected_session().map(|session| session.id.as_str())
    }

    pub(super) fn selected_event(&self) -> Option<&Event> {
        self.events.selected()
    }
}
