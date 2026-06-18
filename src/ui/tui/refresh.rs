// SPDX-License-Identifier: AGPL-3.0-or-later

use super::app::App;
use super::watch::WAL_REFRESH_COALESCE_MS;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::time::{Duration, Instant};

pub(super) struct RefreshSchedule {
    pending: bool,
    pub(super) deadline: Option<Instant>,
    last_refresh: Instant,
}

impl RefreshSchedule {
    pub(super) fn new() -> Self {
        Self {
            pending: false,
            deadline: None,
            last_refresh: Instant::now() - Duration::from_millis(WAL_REFRESH_COALESCE_MS),
        }
    }

    pub(super) fn on_deadline(&mut self, app: &mut App) -> bool {
        self.deadline = None;
        if !self.pending {
            return false;
        }
        self.pending = false;
        let refreshed = app.refresh().is_ok();
        if refreshed {
            self.last_refresh = Instant::now();
        }
        refreshed
    }

    pub(super) fn on_wal(&mut self, app: &mut App, dirty: &AtomicBool) -> bool {
        if !dirty.swap(false, Ordering::AcqRel) {
            return false;
        }
        let ready_at = self.last_refresh + Duration::from_millis(WAL_REFRESH_COALESCE_MS);
        if Instant::now() >= ready_at {
            return self.refresh_now(app);
        }
        self.pending = true;
        self.deadline = Some(ready_at);
        false
    }

    fn refresh_now(&mut self, app: &mut App) -> bool {
        let refreshed = app.refresh().is_ok();
        if refreshed {
            self.last_refresh = Instant::now();
        }
        refreshed
    }
}
