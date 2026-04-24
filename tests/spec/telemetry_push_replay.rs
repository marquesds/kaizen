// SPDX-License-Identifier: AGPL-3.0-or-later
// Telemetry push: exporter-only replay; chunking; no primary POST (see specs/telemetry-push-replay.qnt).
use quint_connect::*;

#[derive(Debug, Default)]
struct TelemetryPushReplayDriver {
    outbox_len: i64,
    primary_posts: i64,
    exporter_fanouts: i64,
    dry_run: bool,
    ready: bool,
    events_queued: i64,
    pending_chunk: i64,
    batches_formed: i64,
    max_per_batch: i64,
}

impl Driver for TelemetryPushReplayDriver {
    type State = ();

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => {
                self.outbox_len = 3;
                self.primary_posts = 0;
                self.exporter_fanouts = 0;
                self.dry_run = false;
                self.ready = true;
                self.events_queued = 0;
                self.pending_chunk = 0;
                self.batches_formed = 0;
                self.max_per_batch = 2;
            }
            init_dry => {
                self.outbox_len = 3;
                self.primary_posts = 0;
                self.exporter_fanouts = 0;
                self.dry_run = true;
                self.ready = true;
                self.events_queued = 0;
                self.pending_chunk = 0;
                self.batches_formed = 0;
                self.max_per_batch = 2;
            }
            init_blocked => {
                self.outbox_len = 3;
                self.primary_posts = 0;
                self.exporter_fanouts = 0;
                self.dry_run = false;
                self.ready = false;
                self.events_queued = 0;
                self.pending_chunk = 0;
                self.batches_formed = 0;
                self.max_per_batch = 2;
            }
            step => {}
            load_window => {
                if self.ready {
                    self.events_queued = 5;
                }
            }
            fanout_round => {
                if self.ready && !self.dry_run && self.events_queued > 0 {
                    self.exporter_fanouts += 1;
                    self.events_queued = 0;
                }
            }
            dry_run_plan => {
                if self.ready && self.dry_run && self.events_queued > 0 {
                    self.events_queued = 0;
                }
            }
            chunking_start_four => {
                self.pending_chunk = 4;
                self.batches_formed = 0;
                self.max_per_batch = 2;
            }
            chunking_flush_one => {
                if self.pending_chunk > 0 {
                    let take = if self.pending_chunk > self.max_per_batch {
                        self.max_per_batch
                    } else {
                        self.pending_chunk
                    };
                    self.pending_chunk -= take;
                    self.batches_formed += 1;
                }
            }
        })
    }
}

#[quint_run(
    spec = "specs/telemetry-push-replay.qnt",
    max_samples = 24,
    max_steps = 12,
    seed = "0x5"
)]
fn telemetry_push_replay_run() -> impl Driver {
    TelemetryPushReplayDriver::default()
}
