// Outbox flush ordering: the sync engine drains one `kind` at a time, using the first pending
// row’s kind. Abstract model: `events` rows drain before `workspace_facts` (queue order: events
// first). Invariants: you cannot flush `workspace_facts` while unflushed `events` remain; each
// successful flush uses a new idempotency attempt (tally only).
module workspace_facts_sync {
  // Pending rows (counts) in arrival order: events, then workspace_facts in this abstraction.
  var n_events: int
  var n_wf: int
  // DrainingEvents: working off the events batch; DrainingWf: events queue empty; Done: all clear.
  var phase: str
  // Count of successful POST attempts (models fresh idempotency key per flush).
  var flush_ok_count: int

  action init: bool = all {
    n_events' = 0,
    n_wf' = 0,
    phase' = "Done",
    flush_ok_count' = 0
  }

  // Both kinds pending; engine will see `events` first.
  action init_mixed: bool = all {
    n_events' = 1,
    n_wf' = 1,
    phase' = "DrainingEvents",
    flush_ok_count' = 0
  }

  // Only workspace_facts remain (e.g. events batch already cleared).
  action init_wf_only: bool = all {
    n_events' = 0,
    n_wf' = 1,
    phase' = "DrainingWf",
    flush_ok_count' = 0
  }

  // Flush one batch of the active kind: events in DrainingEvents, else workspace_facts in DrainingWf.
  action flush_events_batch: bool = all {
    phase == "DrainingEvents",
    n_events > 0,
    n_events' = n_events - 1,
    n_wf' = n_wf,
    phase' = if ((n_events - 1) == 0 and n_wf > 0) "DrainingWf" else (if ((n_events - 1) == 0 and n_wf == 0) "Done" else "DrainingEvents"),
    flush_ok_count' = flush_ok_count + 1
  }

  action flush_wf_batch: bool = all {
    phase == "DrainingWf",
    n_wf > 0,
    n_events' = n_events,
    n_wf' = n_wf - 1,
    phase' = if ((n_wf - 1) == 0) "Done" else "DrainingWf",
    flush_ok_count' = flush_ok_count + 1
  }

  action step: bool = any {
    flush_events_batch,
    flush_wf_batch
  }

  val counts_nonnegative: bool = n_events >= 0 and n_wf >= 0
  // Cannot drain workspace_facts while events still pending in DrainingEvents.
  val wf_waits_for_events: bool = (n_events > 0 and n_wf > 0) implies (phase == "DrainingEvents" or (phase == "DrainingWf" and n_events == 0))

  run events_then_wf_empties_outbox = {
    init_mixed
      .then(flush_events_batch)
      .then(flush_wf_batch)
      .expect(phase == "Done" and n_events == 0 and n_wf == 0 and flush_ok_count == 2)
  }

  run wf_only_single_flush = {
    init_wf_only.then(flush_wf_batch).expect(phase == "Done" and n_wf == 0 and flush_ok_count == 1)
  }
}
