// Sync outbox: bounded batching; retries use a fresh idempotency key (no unbounded dup POSTs
// for the same key in the model — server dedupes keys).
module sync_backpressure {
  var outbox_len: int
  var batch_max: int
  var last_post_ok: bool
  var idem_keys_sent: Set[str]

  action init: bool = all {
    outbox_len' = 0,
    batch_max' = 3,
    last_post_ok' = false,
    idem_keys_sent' = Set()
  }

  action enqueue: bool = all {
    outbox_len' = outbox_len + 1,
    batch_max' = batch_max,
    last_post_ok' = last_post_ok,
    idem_keys_sent' = idem_keys_sent
  }

  // Flush sends at most batch_max items; always uses a fresh key not in idem_keys_sent.
  action flush: bool = {
    nondet fresh = oneOf(Set("k1", "k2", "k3", "k4"))
    val take = if (outbox_len > batch_max) batch_max else outbox_len
    all {
      fresh.in(idem_keys_sent).not(),
      outbox_len' = outbox_len - take,
      batch_max' = batch_max,
      last_post_ok' = true,
      idem_keys_sent' = idem_keys_sent.union(Set(fresh))
    }
  }

  action step: bool = any {
    enqueue,
    flush
  }

  val outbox_non_negative: bool = outbox_len >= 0
  val fresh_key_on_flush: bool = last_post_ok implies idem_keys_sent.size() > 0

  run enqueueThenFlush = {
    init.then(enqueue).then(enqueue).then(flush).expect(outbox_len >= 0)
  }
}
