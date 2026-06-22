// SPDX-License-Identifier: AGPL-3.0-or-later
#[path = "daemon_lifecycle/handshake.rs"]
mod handshake;
#[path = "daemon_lifecycle/process.rs"]
mod process;
#[path = "daemon_lifecycle/restart.rs"]
mod restart;
#[path = "daemon_lifecycle/restore.rs"]
mod restore;
#[path = "daemon_lifecycle/support.rs"]
mod support;
#[cfg(unix)]
#[path = "daemon_lifecycle/timeouts.rs"]
mod timeouts;

use quint_connect::*;

#[quint_run(spec = "specs/daemon-handshake.qnt", max_samples = 20, max_steps = 8)]
fn daemon_handshake_run() -> impl Driver {
    handshake::driver()
}

#[test]
fn daemon_spawn_ingest_query_stop() {
    process::daemon_spawn_ingest_query_stop();
}

#[test]
fn init_starts_daemon_capture_status() {
    process::init_starts_daemon_capture_status();
}

#[test]
fn restart_restores_registered_capture() {
    restart::restart_restores_registered_capture();
}

#[test]
fn stopped_daemon_restart_starts_background() {
    restart::stopped_daemon_restart_starts_background();
}

#[test]
fn startup_waits_for_registered_store_migration() {
    restore::startup_waits_for_registered_store_migration();
}

#[test]
fn startup_readiness_allows_slow_registered_migration() {
    restore::startup_readiness_allows_slow_registered_migration();
}

#[test]
fn scan_failure_status_recovers_without_blocking_other_workspaces() {
    restore::scan_failure_status_recovers_without_blocking_other_workspaces();
}

#[cfg(unix)]
#[test]
fn hung_daemon_ipc_times_out_without_starting_duplicate() {
    timeouts::hung_daemon_ipc_times_out_without_starting_duplicate();
}
