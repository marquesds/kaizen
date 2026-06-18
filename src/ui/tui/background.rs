// SPDX-License-Identifier: AGPL-3.0-or-later

use super::format::now_ms;
use crate::metrics::report;
use crate::metrics::types::MetricsReport;
use crate::store::Store;
use crate::visualization::{VisualizationLimits, VisualizationQuery, VisualizationReport};
use arc_swap::ArcSwapOption;
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tokio::sync::{Notify, mpsc};
use tokio::task::JoinHandle;
use tokio::time::{Duration, Instant, sleep_until};

const REPORT_REFRESH_MIN_MS: u64 = 2_000;
// TUI keeps broader navigation history than Web, but refresh work stays bounded.
const TUI_VISUALIZATION_LIMITS: VisualizationLimits = VisualizationLimits {
    sessions: 100,
    selected_events: 200,
    selected_spans: 200,
    selected_files: 200,
};

pub(super) struct BackgroundWorkers {
    pub(super) report: JoinHandle<()>,
}

impl BackgroundWorkers {
    pub(super) fn shutdown(self) {
        self.report.abort();
    }
}

pub(super) fn spawn_report_worker(
    db_path: PathBuf,
    workspace: String,
    cache: Arc<ArcSwapOption<MetricsReport>>,
    visualization_cache: Arc<ArcSwapOption<VisualizationReport>>,
    dirty: Arc<AtomicBool>,
    notify: Arc<Notify>,
    ui_tx: mpsc::UnboundedSender<()>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut last_run = Instant::now() - Duration::from_millis(REPORT_REFRESH_MIN_MS);
        loop {
            notify.notified().await;
            wait_for_report_slot(last_run).await;
            if !dirty.swap(false, Ordering::AcqRel) {
                continue;
            }
            let next = compute_report(db_path.clone(), workspace.clone()).await;
            last_run = Instant::now();
            if let Ok(Ok((report, visualization))) = next {
                cache.store(Some(Arc::new(report)));
                visualization_cache.store(Some(Arc::new(visualization)));
                let _ = ui_tx.send(());
            }
        }
    })
}

async fn wait_for_report_slot(last_run: Instant) {
    let ready_at = last_run + Duration::from_millis(REPORT_REFRESH_MIN_MS);
    if Instant::now() < ready_at {
        sleep_until(ready_at).await;
    }
}

async fn compute_report(
    db_path: PathBuf,
    workspace: String,
) -> Result<anyhow::Result<(MetricsReport, VisualizationReport)>, tokio::task::JoinError> {
    tokio::task::spawn_blocking(move || {
        let store = Store::open_read_only(&db_path)?;
        let metrics = report::build_report(&store, &workspace, 7)?;
        let visualization =
            crate::visualization::build_report(&store, visualization_query(workspace))?;
        anyhow::Ok((metrics, visualization))
    })
    .await
}

fn visualization_query(workspace: String) -> VisualizationQuery {
    VisualizationQuery {
        workspace,
        selected_session_id: None,
        now_ms: now_ms(),
        include_activity: true,
        select_latest: false,
        limits: TUI_VISUALIZATION_LIMITS,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tui_visualization_query_has_finite_window() {
        let query = visualization_query("/workspace".into());
        assert_eq!(query.limits.sessions, 100);
        assert_eq!(query.limits.selected_events, 200);
        assert_eq!(query.limits.selected_spans, 200);
        assert_eq!(query.limits.selected_files, 200);
        assert!(!query.select_latest);
    }
}
