module event_log_hot {
  // HOT log model: append records, index by session/seq, replay valid prefix.
  var log: List[(str, int)]
  var idx: Map[(str, int), int]
  var corrupt_from: int

  action init = all {
    log' = [],
    idx' = Map(),
    corrupt_from' = -1,
  }

  action append(s: str, q: int) = all {
    corrupt_from == -1,
    log' = log.append((s, q)),
    idx' = idx.set((s, q), log.length()),
    corrupt_from' = corrupt_from,
  }

  action corrupt_tail = all {
    log.length() > 0,
    corrupt_from' = log.length() - 1,
    log' = log,
    idx' = idx,
  }

  val replay_len = if (corrupt_from == -1) log.length() else corrupt_from

  val index_points_inside_log =
    idx.values().forall(i => i >= 0 and i < log.length())

  val replay_is_prefix =
    replay_len >= 0 and replay_len <= log.length()

  run append_lookup = {
    init.then(() => append("s1", 0)).then(() => append("s1", 1))
      .expect(idx.get(("s1", 1)) == Some(1))
  }

  run corrupt_replays_prefix = {
    init.then(() => append("s1", 0)).then(corrupt_tail)
      .expect(replay_len == 0)
  }
}
