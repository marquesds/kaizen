// Ingesting a hook event always materialises/updates a session row (mid-session install),
// a positive `started_at_ms` (Claude can omit `timestamp_ms`), and a real agent source
// (never a literal "unknown" string in storage). Known sources match hook/MCP and tail
// agent strings: Cursor, Claude Code, Codex, Copilot CLI.
module hook_ingest {
  type HookKind = | SessionStart | PreToolUse | PostToolUse | Stop | Other
  type Status = | NotStarted | Running | Waiting | Done

  var status: Status
  var session_exists: bool
  var started_at_ms: int
  var agent: str

  action init: bool = all {
    status' = NotStarted,
    session_exists' = false,
    started_at_ms' = 0,
    agent' = "",
  }

  val sources = Set("cursor", "claude", "codex", "copilot-cli")

  // Any hook ingest records source via nondet so Rust traces can branch on the pick.
  action on_session_start: bool = {
    nondet src = oneOf(sources)
    all {
      status' = Running,
      session_exists' = true,
      started_at_ms' = 1,
      agent' = src,
    }
  }
  action on_pre_tool_use: bool = {
    nondet src = oneOf(sources)
    all {
      status' = Waiting,
      session_exists' = true,
      started_at_ms' = 1,
      agent' = src,
    }
  }
  action on_post_tool_use: bool = {
    nondet src = oneOf(sources)
    all {
      status' = Running,
      session_exists' = true,
      started_at_ms' = 1,
      agent' = src,
    }
  }
  action on_stop: bool = {
    nondet src = oneOf(sources)
    all {
      status' = Done,
      session_exists' = true,
      started_at_ms' = 1,
      agent' = src,
    }
  }
  action on_other: bool = {
    // Unknown hook name: lifecycle may be unchanged, but the session row is still required.
    nondet src = oneOf(sources)
    all {
      status' = status,
      session_exists' = true,
      started_at_ms' = 1,
      agent' = src,
    }
  }

  action step: bool = any {
    on_session_start,
    on_pre_tool_use,
    on_post_tool_use,
    on_stop,
    on_other,
  }

  val received_any_event: bool = (status != NotStarted) or session_exists

  val session_exists_if_any_event: bool = received_any_event implies session_exists

  val started_at_positive: bool = session_exists implies (started_at_ms > 0)

  val agent_is_source_not_unknown: bool =
    session_exists implies agent.in(sources)

  run testHookLifecycle = {
    init
      .then(on_session_start)
      .then(on_pre_tool_use)
      .then(on_post_tool_use)
      .then(on_stop)
      .expect(all {
        status == Done,
        session_exists,
        started_at_ms > 0,
        agent.in(sources),
      })
  }

  run testMidSessionHookInstall = {
    init
      .then(on_pre_tool_use)
      .expect(all {
        session_exists,
        started_at_ms > 0,
        agent.in(sources),
      })
  }
}
