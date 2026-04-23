// Ingest idempotency: new key → 202, duplicate key → 409.
module ingest_idempotency {
  var seen_keys: Set[str]
  var last_status: int
  var last_was_dup: bool

  action init: bool = all {
    seen_keys' = Set(),
    last_status' = 0,
    last_was_dup' = false
  }

  // New key: record it, respond 202.
  action receive_new: bool = {
    nondet key = oneOf(Set("k1", "k2", "k3"))
    all {
      not(key.in(seen_keys)),
      seen_keys' = seen_keys.union(Set(key)),
      last_status' = 202,
      last_was_dup' = false
    }
  }

  // Duplicate key: reject with 409; seen_keys unchanged.
  action receive_dup: bool = {
    nondet key = oneOf(seen_keys)
    all {
      key.in(seen_keys),
      seen_keys' = seen_keys,
      last_status' = 409,
      last_was_dup' = true
    }
  }

  action step: bool = any {
    receive_new,
    receive_dup
  }

  // After a dup action, status is always 409.
  val dup_always_409: bool = last_was_dup implies last_status == 409

  // After a new action, status is always 202.
  val new_always_202: bool =
    (not(last_was_dup) and last_status != 0) implies last_status == 202

  run testDupRejected = {
    init
      .then(receive_new)
      .then(receive_dup)
      .expect(last_status == 409)
  }
}
