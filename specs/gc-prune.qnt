// `kaizen gc`: keep_days==0 refuses prune; positive keep allows prune; vacuum only after prune.
module gc_prune {
  var keepDays: int
  var refused: bool
  var pruned: bool
  var vacuumed: bool

  action init: bool = all {
    keepDays' = 7,
    refused' = false,
    pruned' = false,
    vacuumed' = false
  }

  action set_keep_zero: bool = all {
    keepDays' = 0,
    refused' = refused,
    pruned' = pruned,
    vacuumed' = vacuumed
  }

  action set_keep_positive: bool = all {
    keepDays' = 30,
    refused' = refused,
    pruned' = pruned,
    vacuumed' = vacuumed
  }

  action try_prune: bool = all {
    refused' = (keepDays == 0),
    pruned' = (keepDays > 0),
    keepDays' = keepDays,
    vacuumed' = vacuumed
  }

  action run_vacuum: bool = all {
    pruned,
    vacuumed' = true,
    refused' = refused,
    pruned' = pruned,
    keepDays' = keepDays
  }

  action step: bool = any {
    set_keep_zero,
    set_keep_positive,
    try_prune,
    run_vacuum
  }

  val refuse_implies_no_prune: bool =
    refused implies not(pruned)

  val vacuum_requires_prune: bool =
    vacuumed implies pruned

  run prune_then_vacuum = {
    init
      .then(try_prune)
      .then(run_vacuum)
      .expect(all { pruned, vacuumed, not(refused) })
  }
}
