// Per-request LLM-call quality model.
// Extends llm-proxy.qnt with per-request retry FSM.
// Outcomes: ok | retry | rate_limited | timeout.
// Invariants: retry_count monotonic; event emitted only on terminal outcome;
// retry_count bounded by MAX_RETRIES.
module llm_call_quality {
  type Outcome = | Pending | Inflight | Succeeded | RateLimited | TimedOut | GaveUp

  pure val MAX_RETRIES: int = 5

  var outcome: Outcome
  var retryCount: int
  var eventEmitted: bool

  action init: bool = all {
    outcome'      = Pending,
    retryCount'   = 0,
    eventEmitted' = false
  }

  action send: bool = all {
    outcome == Pending,
    outcome'      = Inflight,
    retryCount'   = retryCount,
    eventEmitted' = eventEmitted
  }

  action ok_response: bool = all {
    outcome == Inflight,
    outcome'      = Succeeded,
    retryCount'   = retryCount,
    eventEmitted' = true
  }

  action rate_limit: bool = all {
    outcome == Inflight,
    outcome'      = RateLimited,
    retryCount'   = retryCount,
    eventEmitted' = eventEmitted
  }

  action timeout: bool = all {
    outcome == Inflight,
    outcome'      = TimedOut,
    retryCount'   = retryCount,
    eventEmitted' = eventEmitted
  }

  // Retry re-enters Inflight from any non-terminal non-Pending state.
  action retry: bool = all {
    outcome != Pending,
    outcome != Inflight,
    outcome != Succeeded,
    outcome != GaveUp,
    retryCount < MAX_RETRIES,
    retryCount'   = retryCount + 1,
    outcome'      = Inflight,
    eventEmitted' = eventEmitted
  }

  // Give up emits a final error event.
  action give_up: bool = all {
    outcome != Pending,
    outcome != Inflight,
    outcome != Succeeded,
    outcome != GaveUp,
    retryCount'   = retryCount,
    outcome'      = GaveUp,
    eventEmitted' = true
  }

  action step: bool = any {
    send,
    ok_response,
    rate_limit,
    timeout,
    retry,
    give_up
  }

  val retry_count_bounded: bool = retryCount >= 0 and retryCount <= MAX_RETRIES

  val event_on_terminal: bool =
    eventEmitted implies (outcome == Succeeded or outcome == GaveUp)

  val retry_increases_count: bool =
    retryCount > 0 implies outcome != Pending

  run happyPath = {
    init
      .then(send)
      .then(ok_response)
      .expect(all {
        outcome == Succeeded,
        retryCount == 0,
        eventEmitted
      })
  }

  run retryOnRateLimit = {
    init
      .then(send)
      .then(rate_limit)
      .then(retry)
      .then(ok_response)
      .expect(all {
        outcome == Succeeded,
        retryCount == 1,
        eventEmitted
      })
  }

  run timeoutThenGiveUp = {
    init
      .then(send)
      .then(timeout)
      .then(give_up)
      .expect(all {
        outcome == GaveUp,
        retryCount == 0,
        eventEmitted
      })
  }
}
