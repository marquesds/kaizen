// Guidance proposals are review-only and never mutate target repositories.

module guidance_candidate {
  var state: int
  // 0 none, 1 proposed, 2 rejected, 3 archived
  var mutated_artifacts: int

  action init: bool = all {
    state' = 0,
    mutated_artifacts' = 0
  }

  action propose: bool = all {
    state == 0,
    state' = 1,
    mutated_artifacts' = 0
  }

  action reject: bool = all {
    state == 1,
    state' = 2,
    mutated_artifacts' = mutated_artifacts
  }

  action archive: bool = all {
    state == 1 or state == 2,
    state' = 3,
    mutated_artifacts' = mutated_artifacts
  }

  action step: bool = all {
    state' = if (state == 0) 1
      else if (state == 1) 2
      else if (state == 2) 3
      else state,
    mutated_artifacts' = mutated_artifacts
  }

  val state_range = state >= 0 and state <= 3
  val repository_read_only = mutated_artifacts == 0

  run proposal_path = {
    init.then(propose)
      .expect(state == 1 and mutated_artifacts == 0)
  }

  run reject_path = {
    init.then(propose).then(reject).then(archive)
      .expect(state == 3 and mutated_artifacts == 0)
  }

  run direct_archive = {
    init.then(propose).then(archive)
      .expect(state == 3 and mutated_artifacts == 0)
  }
}
