// OpenClaw tail ingester: workspace filter guards session admission.
// Each JSONL line either bears a matching cwd, a non-matching cwd, or no cwd.
// A session is admitted only if at least one line contains a cwd matching workspace.
module openclaw_ingest {
  type FilterState = | Scanning | Accepted | Rejected

  var ws_filter: FilterState
  var lines_read: int
  var session_produced: bool

  action init: bool = all {
    ws_filter' = Scanning,
    lines_read' = 0,
    session_produced' = false,
  }

  // Line with a cwd field matching workspace → accept immediately.
  action read_matching_cwd: bool = all {
    ws_filter == Scanning,
    ws_filter' = Accepted,
    lines_read' = lines_read + 1,
    session_produced' = session_produced,
  }

  // Line with no cwd field → keep scanning.
  action read_no_cwd: bool = all {
    ws_filter == Scanning,
    ws_filter' = Scanning,
    lines_read' = lines_read + 1,
    session_produced' = session_produced,
  }

  // Line with cwd NOT matching workspace → reject.
  action read_wrong_cwd: bool = all {
    ws_filter == Scanning,
    ws_filter' = Rejected,
    lines_read' = lines_read + 1,
    session_produced' = session_produced,
  }

  // Emit the session once workspace is confirmed.
  action produce_session: bool = all {
    ws_filter == Accepted,
    session_produced' = true,
    ws_filter' = Accepted,
    lines_read' = lines_read,
  }

  // Drop the session when rejected or unmatched.
  action skip_session: bool = all {
    ws_filter != Accepted,
    session_produced' = false,
    ws_filter' = ws_filter,
    lines_read' = lines_read,
  }

  action step: bool = any {
    read_matching_cwd,
    read_no_cwd,
    read_wrong_cwd,
    produce_session,
    skip_session,
  }

  val admitted_only_when_accepted: bool =
    session_produced implies ws_filter == Accepted

  val rejected_never_produces: bool =
    (ws_filter == Rejected) implies not(session_produced)

  run testMatchingCwdAdmits = {
    init
      .then(read_no_cwd)
      .then(read_matching_cwd)
      .then(produce_session)
      .expect(all { session_produced, ws_filter == Accepted })
  }

  run testWrongCwdRejects = {
    init
      .then(read_wrong_cwd)
      .then(skip_session)
      .expect(all { not(session_produced), ws_filter == Rejected })
  }

  run testNoCwdRejects = {
    init
      .then(read_no_cwd)
      .then(read_no_cwd)
      .then(skip_session)
      .expect(all { not(session_produced), ws_filter == Scanning })
  }
}
