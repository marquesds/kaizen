// Pluggable exporter fan-out: only the Kaizen (primary) POST may clear the outbox.
// Optional secondary sinks never flip durable sync error state. With fail_open,
// a secondary failure does not fail the overall flush; with fail_open false, the
// flush returns an error if any secondary fails (outbox is still not cleared on
// primary failure; when primary succeeds, we model clearing the batch either way
// to match Option A: fan-out in parallel, primary result drives durability only).
module telemetry_exporters {
  var outbox: int
  // Mirrors store sync error: set only on primary (Kaizen) path failure.
  var sync_broken: bool
  // Did the last *attempt* have a secondary sink failure (observability only).
  var secondary_failed: bool
  // When true, a secondary failure does not block success after a primary success.
  var fail_open: bool
  // When primary succeeds, we clear one batch; secondary outcome is orthogonal.
  var last_primary_ok: bool

  action init: bool = all {
    outbox' = 2,
    sync_broken' = false,
    secondary_failed' = false,
    fail_open' = true,
    last_primary_ok' = false
  }

  // Separate starting state: strict secondaries, single pending batch.
  action init_fail_closed: bool = all {
    outbox' = 1,
    sync_broken' = false,
    secondary_failed' = false,
    fail_open' = false,
    last_primary_ok' = false
  }

  // Primary OK + secondary OK: clear outbox, healthy sync.
  action flush_both_ok: bool = all {
    outbox > 0,
    outbox' = outbox - 1,
    sync_broken' = false,
    secondary_failed' = false,
    fail_open' = fail_open,
    last_primary_ok' = true
  }

  // Primary OK, secondary fail, fail_open: still clear; sync not broken.
  action flush_primary_ok_secondary_fail_open: bool = all {
    outbox > 0,
    fail_open,
    outbox' = outbox - 1,
    sync_broken' = false,
    secondary_failed' = true,
    fail_open' = fail_open,
    last_primary_ok' = true
  }

  // Primary OK, secondary fail, not fail_open: do not clear (flush aborts before commit).
  action flush_primary_ok_secondary_fail_strict: bool = all {
    outbox > 0,
    not(fail_open),
    outbox' = outbox,
    sync_broken' = false,
    secondary_failed' = true,
    fail_open' = fail_open,
    last_primary_ok' = true
  }

  // Primary fail: outbox stays; sync broken; secondary irrelevant for durability.
  action flush_primary_fail: bool = all {
    outbox' = outbox,
    sync_broken' = true,
    secondary_failed' = false,
    fail_open' = fail_open,
    last_primary_ok' = false
  }

  action step: bool = any {
    flush_both_ok,
    flush_primary_ok_secondary_fail_open,
    flush_primary_ok_secondary_fail_strict,
    flush_primary_fail
  }

  val outbox_nonnegative: bool = outbox >= 0

  // Durable error only tracks the Kaizen/primary path.
  val primary_failure_sets_sync_broken: bool = sync_broken implies not(last_primary_ok)

  // A secondary failure never sets the durable “sync broken” flag.
  val secondary_never_marks_sync_broken: bool = secondary_failed implies not(sync_broken)

  run happyPath = {
    init.then(flush_both_ok).then(flush_both_ok).expect(outbox == 0 and not(sync_broken))
  }

  run primaryFailSticks = {
    init
      .then(flush_primary_fail)
      .expect(outbox == 2 and sync_broken)
  }

  // Strict: secondary fail prevents commit (outbox unchanged) while not marking sync_broken.
  run strictBlocksWithoutSyncBroken = {
    init_fail_closed
      .then(flush_primary_ok_secondary_fail_strict)
      .expect(outbox == 1 and not(sync_broken) and secondary_failed)
  }
}
