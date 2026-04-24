// Remote event import: stable row key is (team_id, workspace_hash, session_id_hash, event_seq).
// Abstract as one string per row: "h#seq" where h is a session hash token and seq is integer.
// Same key upserted twice in one refresh window still yields one stored row (set dedupe).
module provider_pull_event_keys {
  var cursor: int
  var stored: Set[str]
  var pending: Set[str]
  var txn_open: bool

  action init: bool = all {
    cursor' = 0,
    stored' = Set(),
    pending' = Set(),
    txn_open' = false
  }

  action start_refresh: bool = all {
    not(txn_open),
    txn_open' = true,
    pending' = Set(),
    cursor' = cursor,
    stored' = stored
  }

  // Nondet: import one event key.
  action upsert_event_key: bool = {
    nondet k = oneOf(Set("sh#0", "sh#1", "sh#2"))
    all {
      txn_open,
      pending' = pending.union(Set(k)),
      cursor' = cursor,
      stored' = stored,
      txn_open' = true
    }
  }

  // Deterministic duplicate: same composite key twice in one window.
  action upsert_sh0: bool = all {
    txn_open,
    pending' = pending.union(Set("sh#0")),
    cursor' = cursor,
    stored' = stored,
    txn_open' = true
  }

  action commit: bool = all {
    txn_open,
    stored' = pending,
    cursor' = cursor + 1,
    pending' = Set(),
    txn_open' = false
  }

  action step: bool = any {
    start_refresh,
    upsert_event_key,
    upsert_sh0,
    commit
  }

  val cursor_nonnegative: bool = cursor >= 0
  val closed_means_empty_pending: bool = not(txn_open) implies (pending == Set())

  // Set size never exceeds number of union operations in a window (here: key space is small).
  val pending_dedupes: bool = pending.size() <= 3

  run duplicate_composite_key_one_row = {
    init
      .then(start_refresh)
      .then(upsert_sh0)
      .then(upsert_sh0)
      .then(commit)
      .expect(stored.size() == 1 and cursor == 1)
  }

  run repull_same_key_no_growth = {
    init
      .then(start_refresh)
      .then(upsert_sh0)
      .then(commit)
      .then(start_refresh)
      .then(upsert_sh0)
      .then(commit)
      .expect(stored.size() == 1 and cursor == 2)
  }
}
