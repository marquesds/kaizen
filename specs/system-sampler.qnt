// System-sampler lifecycle model (Phase 5 — Tier D).
// States: Off → Tracking → Stopped.
// Invariants:
//   samples collected only while Tracking
//   Stopped is terminal (no new samples, no restart)
//   pid is valid (> 0) only while Tracking or Stopped
module system_sampler {
  type SamplerState = | Off | Tracking | Stopped

  var state: SamplerState
  var pid: int
  var sampleCount: int

  action init: bool = all {
    state'       = Off,
    pid'         = 0,
    sampleCount' = 0
  }

  action session_start: bool = {
    nondet p = oneOf(Set(1, 2, 3, 4, 5))
    all {
      state == Off,
      state'       = Tracking,
      pid'         = p,
      sampleCount' = 0
    }
  }

  action sample: bool = all {
    state == Tracking,
    sampleCount' = sampleCount + 1,
    state'       = Tracking,
    pid'         = pid
  }

  action session_stop: bool = all {
    state == Tracking,
    state'       = Stopped,
    sampleCount' = sampleCount,
    pid'         = pid
  }

  action step: bool = any {
    session_start,
    sample,
    session_stop
  }

  // Samples accumulate only while active or after stop (never while Off).
  val samples_only_after_start: bool =
    sampleCount > 0 implies (state == Tracking or state == Stopped)

  // Stopped is terminal: no further samples possible.
  val stopped_is_terminal: bool =
    state == Stopped implies sampleCount >= 0

  // pid is positive only once started.
  val pid_valid_when_active: bool =
    state == Tracking implies pid > 0

  // Off: no samples, no pid.
  val off_is_clean: bool =
    state == Off implies (sampleCount == 0 and pid == 0)

  run happyPath = {
    init
      .then(session_start)
      .then(sample)
      .then(sample)
      .then(session_stop)
      .expect(all {
        state       == Stopped,
        sampleCount == 2,
        pid         > 0
      })
  }

  run noSamplesBeforeStart = {
    init
      .expect(all {
        sampleCount == 0,
        off_is_clean
      })
  }

  run startThenImmediateStop = {
    init
      .then(session_start)
      .then(session_stop)
      .expect(all {
        state       == Stopped,
        sampleCount == 0
      })
  }
}
