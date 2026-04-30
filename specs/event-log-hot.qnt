module event_log_hot {
  // HOT log model: append records, index by session/seq, replay valid prefix.
  var log: List[(str, int)]
  var idx: List[((str, int), int)]
  var corrupt_from: int

  action init = all {
    log' = [],
    idx' = [],
    corrupt_from' = -1,
  }

  action append_record(s: str, q: int): bool = all {
    corrupt_from == -1,
    log' = log.append((s, q)),
    idx' = idx.append(((s, q), log.length())),
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
    idx.foldl(true, (ok, e) => ok and e._2 >= 0 and e._2 < log.length())

  val replay_is_prefix =
    replay_len >= 0 and replay_len <= log.length()

  run append_lookup = {
    init.then(append_record("s1", 0)).then(append_record("s1", 1))
      .expect(idx.foldl(false, (found, e) => found or e == (("s1", 1), 1)))
  }

  run corrupt_replays_prefix = {
    init.then(append_record("s1", 0)).then(corrupt_tail)
      .expect(replay_len == 0)
  }
}
