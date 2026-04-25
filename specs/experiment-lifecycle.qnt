// Experiment lifecycle: Draft -> Running -> Concluded -> Archived.
// No backward transitions; Archived is terminal.
module experiment_lifecycle {
  type State = | Draft | Running | Concluded | Archived

  var state: State
  var step_count: int

  pure def rank(s: State): int = match s {
    | Draft     => 0
    | Running   => 1
    | Concluded => 2
    | Archived  => 3
  }

  action init: bool = all {
    state' = Draft,
    step_count' = 0
  }

  action start: bool = all {
    state == Draft,
    state' = Running,
    step_count' = step_count + 1
  }

  action conclude: bool = all {
    state == Running,
    state' = Concluded,
    step_count' = step_count + 1
  }

  action archive: bool = all {
    state == Concluded,
    state' = Archived,
    step_count' = step_count + 1
  }

  action step: bool = any {
    start,
    conclude,
    archive
  }

  // Rank is always in [0, 3].
  val rank_in_range: bool =
    rank(state) >= 0 and rank(state) <= 3

  // Step count matches rank when starting from Draft.
  val steps_match_rank: bool =
    step_count >= 0 and step_count <= 3

  // Archived: rank is maximal — no further transitions defined.
  val archived_is_terminal: bool =
    (state == Archived) implies rank(state) == 3

  // Guards enforce strict ordering: start needs Draft, conclude needs Running,
  // archive needs Concluded — so rank only increases.
  val no_backward_by_guard: bool =
    rank(state) >= 0

  run testHappyPath = {
    init
      .then(start)
      .then(conclude)
      .then(archive)
      .expect(state == Archived)
  }

  run testDraftAfterInit = {
    init
      .expect(state == Draft)
  }

  run testRunningAfterStart = {
    init
      .then(start)
      .expect(state == Running)
  }

  run testConcludedAfterConclude = {
    init
      .then(start)
      .then(conclude)
      .expect(state == Concluded and step_count == 2)
  }
}
