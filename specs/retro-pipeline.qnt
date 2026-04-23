// Retro pipeline: single-flight lock while non-Idle; Writing implies lock held.
module retro_pipeline {
  var phase: str
  var lockHeld: bool

  action init: bool = all {
    phase' = "Idle",
    lockHeld' = false
  }

  action acquire: bool = all {
    phase == "Idle",
    not(lockHeld),
    lockHeld' = true,
    phase' = "Loading"
  }

  action load_done: bool = all {
    phase == "Loading",
    lockHeld' = lockHeld,
    phase' = "Computing"
  }

  action compute_done: bool = all {
    phase == "Computing",
    lockHeld' = lockHeld,
    phase' = "Ranking"
  }

  action rank_done: bool = all {
    phase == "Ranking",
    lockHeld' = lockHeld,
    phase' = "Writing"
  }

  action write_done: bool = all {
    phase == "Writing",
    lockHeld' = false,
    phase' = "Idle"
  }

  action step: bool = any {
    acquire,
    load_done,
    compute_done,
    rank_done,
    write_done
  }

  val writing_implies_lock: bool =
    (phase == "Writing") implies lockHeld

  val idle_unlocked: bool =
    (phase == "Idle") implies not(lockHeld)
}
