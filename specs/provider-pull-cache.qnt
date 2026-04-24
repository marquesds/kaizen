// Remote provider pull into local SQLite cache: monotonic cursor, idempotent upsert by stable
// ids (hashed keys), atomic commit of a refresh window so repeated pulls do not duplicate rows.
module provider_pull_cache {
  var cursor: int
  // Durable rows after the last successful commit.
  var stored: Set[str]
  // Open refresh: rows accumulated for the next commit (replace-window snapshot).
  var pending: Set[str]
  var txn_open: bool

  action init: bool = all {
    cursor' = 0,
    stored' = Set(),
    pending' = Set(),
    txn_open' = false
  }

  // Begin a refresh transaction: new pending window (clear previous pending only).
  action start_refresh: bool = all {
    not(txn_open),
    txn_open' = true,
    pending' = Set(),
    cursor' = cursor,
    stored' = stored
  }

  // Upsert an id into the pending window (dedupe within the window).
  action upsert: bool = {
    nondet id = oneOf(Set("h1", "h2", "h3"))
    all {
      txn_open,
      pending' = pending.union(Set(id)),
      cursor' = cursor,
      stored' = stored,
      txn_open' = true
    }
  }

  // Commit: replace stored with pending, advance cursor, close txn.
  action commit: bool = all {
    txn_open,
    stored' = pending,
    cursor' = cursor + 1,
    pending' = Set(),
    txn_open' = false
  }

  // Abort without moving cursor or touching stored.
  action abort: bool = all {
    txn_open,
    txn_open' = false,
    pending' = Set(),
    cursor' = cursor,
    stored' = stored
  }

  action step: bool = any {
    start_refresh,
    upsert,
    commit,
    abort
  }

  val cursor_nonnegative: bool = cursor >= 0

  // When no open txn, the staging buffer is empty.
  val closed_means_empty_pending: bool = not(txn_open) implies (pending == Set())

  run pull_commit_once = {
    init
      .then(start_refresh)
      .then(upsert)
      .then(commit)
      .expect(cursor == 1 and stored.size() == 1 and not(txn_open))
  }

  // Same logical window pulled twice: still one row per id, cursor moves each time.
  run repull_no_duplicate_rows = {
    init
      .then(start_refresh)
      .then(upsert)
      .then(commit)
      .then(start_refresh)
      .then(upsert)
      .then(commit)
      .expect(cursor == 2 and stored.size() == 1)
  }
}
