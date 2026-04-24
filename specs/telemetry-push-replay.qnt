// CLI `telemetry push`: replay events already in SQLite through pluggable exporters only.
// No Kaizen primary POST, no outbox enqueue/dequeue. Preconditions (v1): valid team salt for
// hashing/redaction and at least one enabled exporter; `--dry-run` plans batches without fan-out.
// Greedy chunking matches sync batch packing: cap count per batch, cap serialized JSON bytes
// (first item in a batch may exceed byte budget alone, same as outbox path).
module telemetry_push_replay {
  var outbox_len: int
  var primary_posts: int
  var exporter_fanouts: int
  var dry_run: bool
  var ready: bool
  var events_queued: int
  var pending_chunk: int
  var batches_formed: int
  var max_per_batch: int

  action init: bool = all {
    outbox_len' = 3,
    primary_posts' = 0,
    exporter_fanouts' = 0,
    dry_run' = false,
    ready' = true,
    events_queued' = 0,
    pending_chunk' = 0,
    batches_formed' = 0,
    max_per_batch' = 2
  }

  action init_dry: bool = all {
    outbox_len' = 3,
    primary_posts' = 0,
    exporter_fanouts' = 0,
    dry_run' = true,
    ready' = true,
    events_queued' = 0,
    pending_chunk' = 0,
    batches_formed' = 0,
    max_per_batch' = 2
  }

  action init_blocked: bool = all {
    outbox_len' = 3,
    primary_posts' = 0,
    exporter_fanouts' = 0,
    dry_run' = false,
    ready' = false,
    events_queued' = 0,
    pending_chunk' = 0,
    batches_formed' = 0,
    max_per_batch' = 2
  }

  // Abstract load from `retro_events_in_window`: no durable writes.
  action load_window: bool = all {
    ready,
    events_queued' = 5,
    outbox_len' = outbox_len,
    primary_posts' = primary_posts,
    exporter_fanouts' = exporter_fanouts,
    dry_run' = dry_run,
    ready' = ready,
    pending_chunk' = pending_chunk,
    batches_formed' = batches_formed,
    max_per_batch' = max_per_batch
  }

  // One push invocation drains the abstract queue via exporter fan-out (batch count folded away).
  action fanout_round: bool = all {
    ready,
    not(dry_run),
    events_queued > 0,
    outbox_len' = outbox_len,
    primary_posts' = primary_posts,
    exporter_fanouts' = exporter_fanouts + 1,
    dry_run' = dry_run,
    ready' = ready,
    events_queued' = 0,
    pending_chunk' = pending_chunk,
    batches_formed' = batches_formed,
    max_per_batch' = max_per_batch
  }

  action dry_run_plan: bool = all {
    ready,
    dry_run,
    events_queued > 0,
    outbox_len' = outbox_len,
    primary_posts' = primary_posts,
    exporter_fanouts' = exporter_fanouts,
    dry_run' = dry_run,
    ready' = ready,
    events_queued' = 0,
    pending_chunk' = pending_chunk,
    batches_formed' = batches_formed,
    max_per_batch' = max_per_batch
  }

  action chunking_start_four: bool = all {
    pending_chunk' = 4,
    batches_formed' = 0,
    max_per_batch' = 2,
    outbox_len' = outbox_len,
    primary_posts' = primary_posts,
    exporter_fanouts' = exporter_fanouts,
    dry_run' = dry_run,
    ready' = ready,
    events_queued' = events_queued
  }

  action chunking_flush_one: bool = all {
    pending_chunk > 0,
    pending_chunk' = pending_chunk - (if (pending_chunk > max_per_batch) max_per_batch else pending_chunk),
    batches_formed' = batches_formed + 1,
    max_per_batch' = max_per_batch,
    outbox_len' = outbox_len,
    primary_posts' = primary_posts,
    exporter_fanouts' = exporter_fanouts,
    dry_run' = dry_run,
    ready' = ready,
    events_queued' = events_queued
  }

  action step: bool = any {
    load_window,
    fanout_round,
    dry_run_plan,
    chunking_start_four,
    chunking_flush_one
  }

  val counts_ok: bool = and {
    outbox_len >= 0,
    exporter_fanouts >= 0,
    events_queued >= 0,
    pending_chunk >= 0,
    batches_formed >= 0,
    max_per_batch >= 1
  }

  val outbox_untouched_after_fanout: bool = exporter_fanouts > 0 implies outbox_len == 3

  val no_primary_post: bool = primary_posts == 0

  val dry_run_skips_fanout: bool = (dry_run and events_queued == 0) implies exporter_fanouts == 0

  run live_push_keeps_outbox = {
    init
      .then(load_window)
      .then(fanout_round)
      .expect(exporter_fanouts == 1 and outbox_len == 3 and primary_posts == 0)
  }

  run dry_run_no_fanout = {
    init_dry.then(load_window).then(dry_run_plan).expect(exporter_fanouts == 0 and events_queued == 0)
  }

  run chunking_splits_on_count_cap = {
    init
      .then(chunking_start_four)
      .then(chunking_flush_one)
      .then(chunking_flush_one)
      .expect(pending_chunk == 0 and batches_formed == 2)
  }
}
