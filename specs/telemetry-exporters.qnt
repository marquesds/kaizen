// Pluggable exporter fan-out: only the Kaizen (primary) POST may clear the outbox.
// Optional secondary sinks never flip durable sync error state. With fail_open,
// a secondary failure does not fail the overall flush; with fail_open false, the
// flush returns an error if any secondary fails (outbox is still not cleared on
// primary failure; when primary succeeds, we model clearing the batch either way
// to match Option A: fan-out in parallel, primary result drives durability only).
//
// N parallel exporter sinks receive the same logical batch. Exactly one
// `query_authority` designates the provider for pull/query-back (posthog, datadog,
// or none). Import windows are single-sourced: no double-import in one refresh
// from multiple authorities.
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
  // Number of parallel exporter sinks in the model (N >= 1).
  var fanout_n: int
  // One designated query authority for pull (mutually exclusive with multi-import).
  var query_authority: str
  // While true, a provider pull transaction is in progress for `import_from` only.
  var import_window_open: bool
  // Which authority a pull refresh targets (or "none" when closed).
  var import_from: str

  action init: bool = all {
    outbox' = 2,
    sync_broken' = false,
    secondary_failed' = false,
    fail_open' = true,
    last_primary_ok' = false,
    fanout_n' = 2,
    query_authority' = "none",
    import_window_open' = false,
    import_from' = "none"
  }

  // Separate starting state: strict secondaries, single pending batch.
  action init_fail_closed: bool = all {
    outbox' = 1,
    sync_broken' = false,
    secondary_failed' = false,
    fail_open' = false,
    last_primary_ok' = false,
    fanout_n' = 2,
    query_authority' = "none",
    import_window_open' = false,
    import_from' = "none"
  }

  // Primary OK + secondary OK: clear outbox, healthy sync.
  action flush_both_ok: bool = all {
    outbox > 0,
    outbox' = outbox - 1,
    sync_broken' = false,
    secondary_failed' = false,
    fail_open' = fail_open,
    last_primary_ok' = true,
    fanout_n' = fanout_n,
    query_authority' = query_authority,
    import_window_open' = import_window_open,
    import_from' = import_from
  }

  // Primary OK, secondary fail, fail_open: still clear; sync not broken.
  action flush_primary_ok_secondary_fail_open: bool = all {
    outbox > 0,
    fail_open,
    outbox' = outbox - 1,
    sync_broken' = false,
    secondary_failed' = true,
    fail_open' = fail_open,
    last_primary_ok' = true,
    fanout_n' = fanout_n,
    query_authority' = query_authority,
    import_window_open' = import_window_open,
    import_from' = import_from
  }

  // Primary OK, secondary fail, not fail_open: do not clear (flush aborts before commit).
  action flush_primary_ok_secondary_fail_strict: bool = all {
    outbox > 0,
    not(fail_open),
    outbox' = outbox,
    sync_broken' = false,
    secondary_failed' = true,
    fail_open' = fail_open,
    last_primary_ok' = true,
    fanout_n' = fanout_n,
    query_authority' = query_authority,
    import_window_open' = import_window_open,
    import_from' = import_from
  }

  // Primary fail: outbox stays; sync broken; secondary irrelevant for durability.
  action flush_primary_fail: bool = all {
    outbox' = outbox,
    sync_broken' = true,
    secondary_failed' = false,
    fail_open' = fail_open,
    last_primary_ok' = false,
    fanout_n' = fanout_n,
    query_authority' = query_authority,
    import_window_open' = import_window_open,
    import_from' = import_from
  }

  // Reconfigure which provider is the single query authority (no pull when "none").
  action set_query_authority: bool = {
    nondet a = oneOf(Set("none", "posthog", "datadog"))
    all {
      not(import_window_open),
      query_authority' = a,
      outbox' = outbox,
      sync_broken' = sync_broken,
      secondary_failed' = secondary_failed,
      fail_open' = fail_open,
      last_primary_ok' = last_primary_ok,
      fanout_n' = fanout_n,
      import_window_open' = false,
      import_from' = "none"
    }
  }

  // Set parallel exporter count (N); used with fan-out to each sink of the same batch.
  action set_fanout: bool = {
    nondet n = oneOf(Set(1, 2, 3))
    all {
      not(import_window_open),
      fanout_n' = n,
      outbox' = outbox,
      sync_broken' = sync_broken,
      secondary_failed' = secondary_failed,
      fail_open' = fail_open,
      last_primary_ok' = last_primary_ok,
      query_authority' = query_authority,
      import_window_open' = false,
      import_from' = "none"
    }
  }

  // Start a single-sourced import window for the configured query authority.
  action import_begin: bool = all {
    not(import_window_open),
    not(query_authority == "none"),
    import_window_open' = true,
    import_from' = query_authority,
    outbox' = outbox,
    sync_broken' = sync_broken,
    secondary_failed' = secondary_failed,
    fail_open' = fail_open,
    last_primary_ok' = last_primary_ok,
    fanout_n' = fanout_n,
    query_authority' = query_authority
  }

  // End the import window; a pull refresh never interleaves two authorities.
  action import_end: bool = all {
    import_window_open,
    import_window_open' = false,
    import_from' = "none",
    outbox' = outbox,
    sync_broken' = sync_broken,
    secondary_failed' = secondary_failed,
    fail_open' = fail_open,
    last_primary_ok' = last_primary_ok,
    fanout_n' = fanout_n,
    query_authority' = query_authority
  }

  action step: bool = any {
    flush_both_ok,
    flush_primary_ok_secondary_fail_open,
    flush_primary_ok_secondary_fail_strict,
    flush_primary_fail,
    set_query_authority,
    set_fanout,
    import_begin,
    import_end
  }

  val outbox_nonnegative: bool = outbox >= 0

  // Durable error only tracks the Kaizen/primary path.
  val primary_failure_sets_sync_broken: bool = sync_broken implies not(last_primary_ok)

  // A secondary failure never sets the durable “sync broken” flag.
  val secondary_never_marks_sync_broken: bool = secondary_failed implies not(sync_broken)

  // Fan-out count is at least one logical sink in the model.
  val fanout_positive: bool = fanout_n >= 1

  // A pull window always targets the designated authority, never a second provider.
  val import_tracks_query_authority: bool =
    import_window_open implies (import_from == query_authority and not(query_authority == "none"))

  // "none" config never has an import window (no double authority for pull).
  val none_has_no_import: bool = (query_authority == "none") implies (not(import_window_open) and import_from == "none")

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
