// SPDX-License-Identifier: AGPL-3.0-or-later
#[path = "daemon_lifecycle/handshake.rs"]
mod handshake;
#[path = "daemon_lifecycle/process.rs"]
mod process;

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
