// SPDX-License-Identifier: AGPL-3.0-or-later
//! Transcript scanner health projection.

use super::capture_status::component;
use crate::ipc::{CaptureComponent, CaptureComponentStatus, CaptureStatus};

const SCANNER: &str = "transcript-scanner";
const ERROR_PREFIX: &str = "transcript-scanner:";

pub(super) fn pending() -> CaptureComponent {
    component(
        SCANNER,
        CaptureComponentStatus::Partial,
        Some("initial scan pending".into()),
    )
}

pub(super) fn update(capture: &mut CaptureStatus, error: Option<String>) {
    capture.watchers.retain(|watcher| watcher.name != SCANNER);
    capture.watchers.push(health_component(error.as_deref()));
    capture
        .errors
        .retain(|message| !message.starts_with(ERROR_PREFIX));
    if let Some(message) = error {
        capture.errors.push(format!("{ERROR_PREFIX} {message}"));
    }
}

fn health_component(error: Option<&str>) -> CaptureComponent {
    match error {
        Some(message) => component(SCANNER, CaptureComponentStatus::Error, Some(message.into())),
        None => component(SCANNER, CaptureComponentStatus::Ready, None),
    }
}
