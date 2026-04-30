// Summary / MCP rollup: when sessions exist but stored cost is zero, UX must surface cost_note (honest gap vs inventing spend).
module summary_cost_rollup {
  var phase: str
  var session_count: int
  var total_cost_usd_e6: int
  var cost_note_emitted: bool

  action init: bool = all {
    phase' = "Idle",
    session_count' = 0,
    total_cost_usd_e6' = 0,
    cost_note_emitted' = false
  }

  action load_no_sessions: bool = all {
    phase == "Idle",
    phase' = "Ready",
    session_count' = 0,
    total_cost_usd_e6' = 0,
    cost_note_emitted' = cost_note_emitted
  }

  action load_sessions_with_cost: bool = all {
    phase == "Idle",
    phase' = "Ready",
    session_count' = 2,
    total_cost_usd_e6' = 100,
    cost_note_emitted' = cost_note_emitted
  }

  action load_sessions_zero_cost: bool = all {
    phase == "Idle",
    phase' = "Ready",
    session_count' = 3,
    total_cost_usd_e6' = 0,
    cost_note_emitted' = cost_note_emitted
  }

  action emit: bool = all {
    phase == "Ready",
    phase' = "Done",
    session_count' = session_count,
    total_cost_usd_e6' = total_cost_usd_e6,
    cost_note_emitted' = session_count > 0 and total_cost_usd_e6 == 0
  }

  action step: bool = any {
    load_no_sessions,
    load_sessions_with_cost,
    load_sessions_zero_cost,
    emit
  }

  val zero_cost_note_rule: bool =
    (phase == "Done" and session_count > 0 and total_cost_usd_e6 == 0) implies cost_note_emitted

  val no_false_positive: bool =
    (phase == "Done" and (session_count == 0 or total_cost_usd_e6 > 0)) implies not(cost_note_emitted)

  run needs_note = {
    init
      .then(load_sessions_zero_cost)
      .then(emit)
      .expect(all { phase == "Done", cost_note_emitted })
  }

  run skips_note_no_sessions = {
    init
      .then(load_no_sessions)
      .then(emit)
      .expect(all { phase == "Done", not(cost_note_emitted) })
  }

  run skips_note_with_cost = {
    init
      .then(load_sessions_with_cost)
      .then(emit)
      .expect(all { phase == "Done", not(cost_note_emitted) })
  }
}
