// `kaizen upgrade`: detect install method → run one upgrade action → exit.
// Invariant: no action executes before method is known.
module upgrade {
  // 0=idle, 1=detecting, 2=executing, 3=done
  var phase: int
  // 0=unknown, 1=homebrew, 2=release-binary, 3=source-cargo
  var method: int
  // upgrade action outcome
  var last_ok: bool

  action init: bool = all {
    phase' = 0,
    method' = 0,
    last_ok' = false,
  }

  action begin_detect: bool = all {
    phase == 0,
    phase' = 1,
    method' = method,
    last_ok' = last_ok,
  }

  action detect_homebrew: bool = all {
    phase == 1,
    phase' = 2,
    method' = 1,
    last_ok' = last_ok,
  }

  action detect_release: bool = all {
    phase == 1,
    phase' = 2,
    method' = 2,
    last_ok' = last_ok,
  }

  action detect_source: bool = all {
    phase == 1,
    phase' = 2,
    method' = 3,
    last_ok' = last_ok,
  }

  action execute_ok: bool = all {
    phase == 2,
    method != 0,
    phase' = 3,
    method' = method,
    last_ok' = true,
  }

  action execute_fail: bool = all {
    phase == 2,
    method != 0,
    phase' = 3,
    method' = method,
    last_ok' = false,
  }

  action step: bool = any {
    begin_detect,
    detect_homebrew,
    detect_release,
    detect_source,
    execute_ok,
    execute_fail,
  }

  val phase_range: bool = (phase >= 0 and phase <= 3)
  val method_range: bool = (method >= 0 and method <= 3)
  // Core invariant: no action without a resolved method
  val no_execute_without_method: bool =
    (phase == 2 or phase == 3) implies (method != 0)

  run happyHomebrew = {
    init
      .then(begin_detect)
      .then(detect_homebrew)
      .then(execute_ok)
      .expect(phase == 3 and method == 1 and last_ok)
  }

  run happyRelease = {
    init
      .then(begin_detect)
      .then(detect_release)
      .then(execute_ok)
      .expect(phase == 3 and method == 2 and last_ok)
  }

  run happySource = {
    init
      .then(begin_detect)
      .then(detect_source)
      .then(execute_ok)
      .expect(phase == 3 and method == 3 and last_ok)
  }

  run executeFail = {
    init
      .then(begin_detect)
      .then(detect_release)
      .then(execute_fail)
      .expect(phase == 3 and not(last_ok))
  }
}
