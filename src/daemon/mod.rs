// SPDX-License-Identifier: AGPL-3.0-or-later
//! Local daemon client/lifecycle API.

mod background;
mod capture_status;
mod client;
mod lifecycle;
mod proxy_task;
mod scanner_health;
mod scanner_task;
mod server;
mod supervisor;
mod worker;

pub use background::{BackgroundStart, restart_background, start_background, start_background_for};
pub use client::{
    begin_observed_session_blocking, ensure_capture_blocking, ensure_proxy_blocking,
    hello_blocking, request_blocking,
};
pub use lifecycle::{
    DaemonStatusOutcome, RuntimePaths, enabled, ensure_running, ensure_running_for, runtime_paths,
    runtime_paths_for, start_foreground, status_outcome, stop, try_status,
};
