// SPDX-License-Identifier: AGPL-3.0-or-later
//! Capture health rows returned by daemon IPC.

use crate::ipc::{CaptureComponent, CaptureComponentStatus, CaptureStatus};
use anyhow::Result;
use std::path::Path;

pub(super) fn base_status(ws: &Path, deep: bool) -> CaptureStatus {
    CaptureStatus {
        workspace: ws.to_string_lossy().to_string(),
        deep,
        hooks: hook_components(ws),
        watchers: Vec::new(),
        proxies: Vec::new(),
        errors: Vec::new(),
    }
}

pub(super) fn component(
    name: &str,
    status: CaptureComponentStatus,
    detail: Option<String>,
) -> CaptureComponent {
    CaptureComponent {
        name: name.into(),
        status,
        detail,
    }
}

fn hook_components(ws: &Path) -> Vec<CaptureComponent> {
    [
        (
            "cursor-hooks",
            crate::shell::init::cursor_kaizen_hook_wiring(ws),
        ),
        (
            "claude-hooks",
            crate::shell::init::claude_kaizen_hook_wiring(ws),
        ),
        (
            "openclaw-hooks",
            crate::shell::init::openclaw_kaizen_hook_wiring(ws),
        ),
    ]
    .into_iter()
    .map(|(name, status)| hook_component(name, status))
    .collect()
}

fn hook_component(name: &str, status: Result<Option<bool>, String>) -> CaptureComponent {
    match status {
        Ok(Some(true)) => component(name, CaptureComponentStatus::Ready, None),
        Ok(Some(false)) => component(name, CaptureComponentStatus::Partial, None),
        Ok(None) => component(name, CaptureComponentStatus::Unsupported, None),
        Err(err) => component(name, CaptureComponentStatus::Error, Some(err)),
    }
}
