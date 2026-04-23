// kaizen local LLM HTTP proxy: forward to upstream, record EventSource::Proxy in SQLite.
// Abstract: no HTTP/SSE. Invariants: bind before forwards, one telemetry unit per
// completed outcome, monotonic eventSeq, unbound client hits advance rejects only.
module llm_proxy {
  var listening: bool
  var eventSeq: int
  var successForwards: int
  var errorForwards: int
  var unboundRejects: int

  action init: bool = all {
    listening' = false,
    eventSeq' = 0,
    successForwards' = 0,
    errorForwards' = 0,
    unboundRejects' = 0
  }

  action start_listen: bool = all {
    not(listening),
    listening' = true,
    eventSeq' = eventSeq,
    successForwards' = successForwards,
    errorForwards' = errorForwards,
    unboundRejects' = unboundRejects
  }

  action forward_ok: bool = all {
    listening,
    eventSeq' = eventSeq + 1,
    successForwards' = successForwards + 1,
    errorForwards' = errorForwards,
    listening' = true,
    unboundRejects' = unboundRejects
  }

  action forward_err: bool = all {
    listening,
    eventSeq' = eventSeq + 1,
    successForwards' = successForwards,
    errorForwards' = errorForwards + 1,
    listening' = true,
    unboundRejects' = unboundRejects
  }

  action reject_unbound: bool = all {
    not(listening),
    unboundRejects' = unboundRejects + 1,
    listening' = false,
    eventSeq' = eventSeq,
    successForwards' = successForwards,
    errorForwards' = errorForwards
  }

  action step: bool = any {
    start_listen,
    forward_ok,
    forward_err,
    reject_unbound
  }

  val one_row_per_outcome: bool = eventSeq == successForwards + errorForwards

  val unbound_does_not_touch_seq: bool = not(listening) implies
    (eventSeq == 0
      and successForwards == 0
      and errorForwards == 0)

  val seq_non_negative: bool = eventSeq >= 0

  run happyPath = {
    init
      .then(start_listen)
      .then(forward_ok)
      .then(forward_err)
      .expect(
        all {
          one_row_per_outcome,
          listening
        }
      )
  }

  run rejectBeforeListen = {
    init
      .then(reject_unbound)
      .then(start_listen)
      .then(forward_ok)
      .expect(
        all {
          unboundRejects == 1,
          successForwards == 1,
          one_row_per_outcome
        }
      )
  }
}
