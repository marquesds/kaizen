// Prompt/system-prompt version tracking lifecycle.
// SessionStart captures a fingerprint and stores it.
// SessionStop re-captures; if the fingerprint changed, a prompt_changed event fires.
// Fingerprints are abstract integers: 0 = none, 1 = fp_a, 2 = fp_b.
// session_state: 0 = Idle, 1 = Running.
module prompt_tracking {
  var session_state: int
  var stored_fingerprint: int
  var stop_fingerprint: int
  var prompt_changed_fired: bool

  action init: bool = all {
    session_state' = 0,
    stored_fingerprint' = 0,
    stop_fingerprint' = 0,
    prompt_changed_fired' = false,
  }

  // SessionStart with fingerprint 1 (fp_a).
  action start_a: bool = all {
    session_state == 0,
    session_state' = 1,
    stored_fingerprint' = 1,
    stop_fingerprint' = stop_fingerprint,
    prompt_changed_fired' = false,
  }

  // SessionStart with fingerprint 2 (fp_b).
  action start_b: bool = all {
    session_state == 0,
    session_state' = 1,
    stored_fingerprint' = 2,
    stop_fingerprint' = stop_fingerprint,
    prompt_changed_fired' = false,
  }

  // SessionStop: re-capture with fingerprint 1.
  action stop_a: bool = all {
    session_state == 1,
    session_state' = 0,
    stored_fingerprint' = stored_fingerprint,
    stop_fingerprint' = 1,
    prompt_changed_fired' = (1 != stored_fingerprint),
  }

  // SessionStop: re-capture with fingerprint 2.
  action stop_b: bool = all {
    session_state == 1,
    session_state' = 0,
    stored_fingerprint' = stored_fingerprint,
    stop_fingerprint' = 2,
    prompt_changed_fired' = (2 != stored_fingerprint),
  }

  action step: bool = any {
    start_a,
    start_b,
    stop_a,
    stop_b,
  }

  val stored_set_when_running: bool = (session_state == 1) implies stored_fingerprint > 0
  val no_change_from_idle: bool = prompt_changed_fired implies stored_fingerprint > 0
  val changed_only_when_idle: bool = prompt_changed_fired implies session_state == 0
  val stop_fp_valid: bool = (stop_fingerprint > 0) implies session_state == 0

  run same_prompt_no_event = {
    init
      .then(start_a)
      .then(stop_a)
      .expect(not(prompt_changed_fired))
  }

  run changed_prompt_fires = {
    init
      .then(start_a)
      .then(stop_b)
      .expect(prompt_changed_fired)
  }

  run restart_clears_changed = {
    init
      .then(start_a)
      .then(stop_b)
      .then(start_b)
      .expect(not(prompt_changed_fired))
  }

  run second_session_same_no_event = {
    init
      .then(start_a)
      .then(stop_b)
      .then(start_b)
      .then(stop_b)
      .expect(not(prompt_changed_fired))
  }

  run start_sets_stored = {
    init
      .then(start_a)
      .expect(stored_fingerprint == 1)
  }
}
