// Guidance score validation gate.
// Gate codes: 0 no_data, 1 needs_more_validation, 2 stable, 3 regression.

module guidance_score {
  var train_sessions: int
  var validation_sessions: int
  var train_score: int
  var validation_score: int
  var gate: int

  pure def min_validation(min_sessions: int): int =
    if (min_sessions / 3 < 1) 1 else min_sessions / 3

  pure def calc_gate(ts: int, vs: int, tr: int, va: int, min_sessions: int): int =
    if (ts + vs == 0) 0
    else if (ts == 0 or vs < min_validation(min_sessions)) 1
    else if (va + 10 < tr) 3
    else 2

  action init: bool = all {
    train_sessions' = 0,
    validation_sessions' = 0,
    train_score' = 0,
    validation_score' = 0,
    gate' = 0
  }

  action train_high: bool = all {
    train_sessions' = 20,
    validation_sessions' = validation_sessions,
    train_score' = 90,
    validation_score' = validation_score,
    gate' = calc_gate(20, validation_sessions, 90, validation_score, 30)
  }

  action validation_low: bool = all {
    train_sessions' = train_sessions,
    validation_sessions' = 10,
    train_score' = train_score,
    validation_score' = 60,
    gate' = calc_gate(train_sessions, 10, train_score, 60, 30)
  }

  action validation_high: bool = all {
    train_sessions' = train_sessions,
    validation_sessions' = 10,
    train_score' = train_score,
    validation_score' = 88,
    gate' = calc_gate(train_sessions, 10, train_score, 88, 30)
  }

  action validation_tiny: bool = all {
    train_sessions' = train_sessions,
    validation_sessions' = 3,
    train_score' = train_score,
    validation_score' = validation_score,
    gate' = calc_gate(train_sessions, 3, train_score, validation_score, 30)
  }

  action step: bool = any { train_high, validation_low, validation_high, validation_tiny }

  val sessions_nonnegative: bool = train_sessions >= 0 and validation_sessions >= 0
  val scores_bounded: bool =
    train_score >= 0 and train_score <= 100 and validation_score >= 0 and validation_score <= 100
  val gate_matches_inputs: bool =
    gate == calc_gate(train_sessions, validation_sessions, train_score, validation_score, 30)
  val regression_requires_validation: bool =
    gate == 3 implies validation_sessions >= 10 and validation_score + 10 < train_score

  run no_data = {
    init.expect(gate == 0)
  }

  run insufficient_validation = {
    init.then(train_high).then(validation_tiny)
      .expect(gate == 1)
  }

  run held_out_regression = {
    init.then(train_high).then(validation_low)
      .expect(gate == 3)
  }

  run held_out_stable = {
    init.then(train_high).then(validation_high)
      .expect(gate == 2)
  }
}
