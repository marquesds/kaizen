// SPDX-License-Identifier: AGPL-3.0-or-later

use super::*;
use crate::store::Store;
use anyhow::Result;
use std::path::Path;

impl App {
    pub(in crate::ui::tui) fn open(
        workspace: &Path,
        metrics_cache: Arc<ArcSwapOption<MetricsReport>>,
        visualization_cache: Arc<ArcSwapOption<VisualizationReport>>,
        report_dirty: Arc<AtomicBool>,
        report_notify: Arc<Notify>,
    ) -> Result<Self> {
        let db = crate::core::workspace::db_path(workspace)?;
        Store::open(&db)?;
        let (store_tx, store_rx) = super::super::worker::spawn_store_worker(db);
        let mut app = Self::build(
            workspace.to_string_lossy().to_string(),
            metrics_cache,
            visualization_cache,
            report_dirty,
            report_notify,
            store_tx,
            store_rx,
        );
        app.mark_report_dirty();
        app.request_session_pages();
        app.request_feedback_for_viewport();
        Ok(app)
    }

    fn build(
        workspace: String,
        metrics_cache: Arc<ArcSwapOption<MetricsReport>>,
        visualization_cache: Arc<ArcSwapOption<VisualizationReport>>,
        report_dirty: Arc<AtomicBool>,
        report_notify: Arc<Notify>,
        store_tx: mpsc::UnboundedSender<StoreRequest>,
        store_rx: mpsc::UnboundedReceiver<StoreResponse>,
    ) -> Self {
        let metrics = metrics_cache.load_full().as_deref().cloned();
        let visualization = visualization_cache.load_full().as_deref().cloned();
        Self {
            sessions: SessionView::new(),
            events: EventView::new(),
            detail_state: DetailState::Idle,
            agent_filter: String::new(),
            filter_mode: false,
            filter_buf: String::new(),
            clipboard_note: String::new(),
            left_focus: true,
            show_help: false,
            detail: false,
            show_metrics: false,
            metrics,
            visualization,
            metrics_cache,
            visualization_cache,
            report_dirty,
            report_notify,
            pulse: false,
            workspace,
            store_tx,
            store_rx,
            feedback_scores: HashMap::new(),
            feedback_token: 0,
            detail_token: 0,
            last_session_id: None,
            session_viewport_height: DEFAULT_VIEWPORT_HEIGHT,
            event_viewport_height: DEFAULT_VIEWPORT_HEIGHT,
            error_note: String::new(),
        }
    }

    #[cfg(test)]
    pub(in crate::ui::tui) fn test() -> Self {
        let (store_tx, _requests) = mpsc::unbounded_channel();
        let (_responses, store_rx) = mpsc::unbounded_channel();
        Self::build(
            "/ws".to_string(),
            Arc::new(ArcSwapOption::from(None)),
            Arc::new(ArcSwapOption::from(None)),
            Arc::new(AtomicBool::new(false)),
            Arc::new(Notify::new()),
            store_tx,
            store_rx,
        )
    }
}
