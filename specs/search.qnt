// Search index lifecycle.
// Queryable docs refine retained persisted events after commit or reindex.

module search {
  type Event = int

  var persisted: Set[Event]
  var pending: Set[Event]
  var indexed: Set[Event]
  var fallback: bool

  action init: bool = all {
    persisted' = Set(),
    pending' = Set(),
    indexed' = Set(),
    fallback' = false
  }

  action append_event: bool = all {
    persisted' = persisted.union(Set(1)),
    pending' = pending.union(Set(1)),
    indexed' = indexed,
    fallback' = fallback
  }

  action commit: bool = all {
    persisted' = persisted,
    pending' = Set(),
    indexed' = indexed.union(pending),
    fallback' = fallback
  }

  action reindex: bool = all {
    persisted' = persisted,
    pending' = Set(),
    indexed' = persisted,
    fallback' = false
  }

  action delete_old: bool = all {
    persisted' = Set(),
    pending' = Set(),
    indexed' = Set(),
    fallback' = fallback
  }

  action corrupt: bool = all {
    persisted' = persisted,
    pending' = pending,
    indexed' = indexed,
    fallback' = true
  }

  action step: bool = any {
    append_event,
    commit,
    reindex,
    delete_old,
    corrupt
  }

  val queryable_refines_persisted: bool = indexed.forall(e => persisted.contains(e))
  val fallback_reads_persisted: bool = fallback implies persisted.contains(1) == persisted.contains(1)

  run append_commit_queryable = {
    init.then(append_event).then(commit).expect(queryable_refines_persisted)
  }

  run reindex_restores_parity = {
    init.then(append_event).then(reindex).expect(indexed == persisted)
  }

  run delete_removes_docs = {
    init.then(append_event).then(commit).then(delete_old).expect(indexed == Set())
  }
}
