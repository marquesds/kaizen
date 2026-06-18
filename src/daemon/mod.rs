// SPDX-License-Identifier: AGPL-3.0-or-later
//! Local daemon client/lifecycle API.

mod background;
mod capture_status;
mod client;
mod lifecycle;
mod proxy_task;
mod scanner_task;
mod server;
mod supervisor;
mod worker;

pub use background::{BackgroundStart, start_background};
pub use client::{
    begin_observed_session_blocking, ensure_capture_blocking, ensure_proxy_blocking,
    hello_blocking, request_blocking,
};
pub use lifecycle::{
    DaemonStatusOutcome, RuntimePaths, enabled, ensure_running, runtime_paths, start_foreground,
    status_outcome, stop, try_status,
};
