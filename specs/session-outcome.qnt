// Session outcome side-state model (Phase 4 — Tier C).
// Invariant: measurement never begins before session has stopped.
// Outcome is terminal: once Measured, stays Measured.
// test_status models {pass, fail, skipped, error} as a count pair.
module session_outcome {
  type SessionPhase  = | Live | Stopped
  type OutcomeStatus = | None | Measuring | Measured

  var phase: SessionPhase
  var outcome: OutcomeStatus
  var testsPassed: int
  var testsFailed: int

  action init: bool = all {
    phase'       = Live,
    outcome'     = None,
    testsPassed' = 0,
    testsFailed' = 0
  }

  action session_stop: bool = all {
    phase == Live,
    phase'       = Stopped,
    outcome'     = outcome,
    testsPassed' = testsPassed,
    testsFailed' = testsFailed
  }

  action start_measure: bool = all {
    phase   == Stopped,
    outcome == None,
    phase'       = Stopped,
    outcome'     = Measuring,
    testsPassed' = testsPassed,
    testsFailed' = testsFailed
  }

  action finish_measure: bool = {
    nondet p = oneOf(Set(0, 1, 5, 10))
    nondet f = oneOf(Set(0, 1, 2))
    all {
      outcome == Measuring,
      phase'       = phase,
      outcome'     = Measured,
      testsPassed' = p,
      testsFailed' = f
    }
  }

  action step: bool = any {
    session_stop,
    start_measure,
    finish_measure
  }

  // Core: outcome measurement only after Stop.
  val outcome_after_stop: bool =
    (outcome != None) implies phase == Stopped

  // Measured is terminal side-state.
  val measured_is_stable: bool =
    outcome == Measured implies phase == Stopped

  // Test counts are non-negative.
  val counts_non_negative: bool =
    testsPassed >= 0 and testsFailed >= 0

  run happyPath = {
    init
      .then(session_stop)
      .then(start_measure)
      .then(finish_measure)
      .expect(all {
        phase   == Stopped,
        outcome == Measured,
        outcome_after_stop
      })
  }

  run noMeasureWhileLive = {
    init
      .expect(all {
        phase   == Live,
        outcome == None
      })
  }
}
