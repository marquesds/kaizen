// Guidance candidate lifecycle.
// Proposed candidate may be applied, rejected, validated, or archived.

module guidance_candidate {
  var state: int
  // 0 none, 1 proposed, 2 applied, 3 validated, 4 rejected, 5 archived
  var was_applied: bool
  var backup_saved: bool
  var mutated_artifacts: int
  var experiment_attached: bool

  action init: bool = all {
    state' = 0,
    was_applied' = false,
    backup_saved' = false,
    mutated_artifacts' = 0,
    experiment_attached' = false
  }

  action propose: bool = all {
    state == 0,
    state' = 1,
    was_applied' = false,
    backup_saved' = false,
    mutated_artifacts' = 0,
    experiment_attached' = false
  }

  action apply: bool = all {
    state == 1,
    state' = 2,
    was_applied' = true,
    backup_saved' = true,
    mutated_artifacts' = 1,
    experiment_attached' = true
  }

  action reject: bool = all {
    state == 1 or state == 2,
    state' = 4,
    was_applied' = was_applied,
    backup_saved' = backup_saved,
    mutated_artifacts' = mutated_artifacts,
    experiment_attached' = experiment_attached
  }

  action validate: bool = all {
    state == 2,
    state' = 3,
    was_applied' = was_applied,
    backup_saved' = backup_saved,
    mutated_artifacts' = mutated_artifacts,
    experiment_attached' = experiment_attached
  }

  action validate_reject: bool = all {
    state == 2,
    state' = 4,
    was_applied' = was_applied,
    backup_saved' = backup_saved,
    mutated_artifacts' = mutated_artifacts,
    experiment_attached' = experiment_attached
  }

  action validate_insufficient: bool = all {
    state == 2,
    state' = 2,
    was_applied' = was_applied,
    backup_saved' = backup_saved,
    mutated_artifacts' = mutated_artifacts,
    experiment_attached' = experiment_attached
  }

  action archive: bool = all {
    state == 3 or state == 4,
    state' = 5,
    was_applied' = was_applied,
    backup_saved' = backup_saved,
    mutated_artifacts' = mutated_artifacts,
    experiment_attached' = experiment_attached
  }

  action step: bool = all {
    state' = if (state == 0) 1
      else if (state == 1) 2
      else if (state == 2) 3
      else if (state == 3 or state == 4) 5
      else state,
    was_applied' = if (state == 1) true else was_applied,
    backup_saved' = if (state == 1) true else backup_saved,
    mutated_artifacts' = if (state == 1) 1 else mutated_artifacts,
    experiment_attached' = if (state == 1) true else experiment_attached
  }

  val state_range = state >= 0 and state <= 5
  val mutation_bounded = mutated_artifacts == 0 or mutated_artifacts == 1
  val mutation_requires_backup = mutated_artifacts > 0 implies backup_saved
  val applied_has_experiment =
    was_applied implies (backup_saved and mutated_artifacts == 1 and experiment_attached)
  val validated_requires_apply = state == 3 implies was_applied

  run happy_path = {
    init.then(propose).then(apply).then(validate).then(archive)
      .expect(state == 5)
  }

  run reject_path = {
    init.then(propose).then(reject).then(archive)
      .expect(state == 5)
  }

  run insufficient_path = {
    init.then(propose).then(apply).then(validate_insufficient)
      .expect(state == 2)
  }

  run apply_safety = {
    init.then(propose).then(apply)
      .expect(was_applied and backup_saved and mutated_artifacts == 1 and experiment_attached)
  }
}
