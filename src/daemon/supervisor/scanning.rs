// SPDX-License-Identifier: AGPL-3.0-or-later
//! Daemon scanner lifecycle and health projection.

use super::{Supervisor, workspace_path};
use crate::daemon::capture_status::base_status;
use crate::daemon::scanner_health;
use crate::daemon::scanner_task::{scan_once, scanner_loop};
use crate::ipc::CaptureStatus;
use anyhow::Result;
use std::path::{Path, PathBuf};

impl Supervisor {
    pub(in crate::daemon) async fn restore_registered(&self) -> Result<()> {
        for workspace in crate::core::machine_registry::list_paths()? {
            self.ensure_capture(workspace.to_string_lossy().to_string(), false)
                .await;
        }
        Ok(())
    }

    pub(in crate::daemon) async fn ensure_capture(
        &self,
        workspace: String,
        deep: bool,
    ) -> CaptureStatus {
        let ws = workspace_path(&workspace);
        let status = self.pending_capture(&ws, deep).await;
        let key = status.workspace.clone();
        self.remember_capture(status.clone());
        self.ensure_scanner(ws).await;
        self.capture(&key).unwrap_or(status)
    }

    async fn pending_capture(&self, ws: &Path, deep: bool) -> CaptureStatus {
        let mut status = base_status(ws, deep);
        status.watchers.push(scanner_health::pending());
        if deep {
            self.add_deep_capture(ws, &mut status).await;
        }
        status
    }

    async fn ensure_scanner(&self, ws: PathBuf) {
        let spawn = self.reserve_scanner(&ws);
        self.record_scan_result(&ws, scan_once(ws.clone()).await);
        if spawn {
            tokio::spawn(scanner_loop(ws, self.clone()));
        }
    }

    pub(in crate::daemon) fn record_scan_result(&self, ws: &Path, result: Result<()>) {
        let error = result.err().map(|err| format!("{err:#}"));
        log_error(ws, error.as_deref());
        self.update_scan_health(ws, error);
    }

    fn update_scan_health(&self, ws: &Path, error: Option<String>) {
        let key = ws.to_string_lossy();
        if let Ok(mut state) = self.inner.lock()
            && let Some(capture) = state.captures.get_mut(key.as_ref())
        {
            scanner_health::update(capture, error);
        }
    }

    fn reserve_scanner(&self, ws: &Path) -> bool {
        let key = ws.to_string_lossy().to_string();
        self.inner
            .lock()
            .map(|mut state| state.scanners.insert(key))
            .unwrap_or(false)
    }

    fn capture(&self, workspace: &str) -> Option<CaptureStatus> {
        self.inner.lock().ok()?.captures.get(workspace).cloned()
    }
}

fn log_error(ws: &Path, error: Option<&str>) {
    if let Some(error) = error {
        tracing::warn!(workspace = %ws.display(), %error, "daemon scanner failed");
    }
}
