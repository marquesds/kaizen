module hook_ingest {
  type HookKind = | SessionStart | PreToolUse | PostToolUse | Stop | Other
  type Status = | NotStarted | Running | Waiting | Done

  var status: Status

  action init: bool = { status' = NotStarted }

  action on_session_start: bool = all { status == NotStarted, status' = Running }
  action on_pre_tool_use:  bool = all { status == Running,    status' = Waiting }
  action on_post_tool_use: bool = all { status == Waiting,    status' = Running }
  action on_stop:          bool = all { status != Done,       status' = Done }
  action on_other:         bool = all { status' = status }

  action step: bool = any {
    on_session_start,
    on_pre_tool_use,
    on_post_tool_use,
    on_stop,
    on_other
  }

  val done_is_terminal: bool = status == Done implies status == Done

  run testHookLifecycle = {
    init
      .then(on_session_start)
      .then(on_pre_tool_use)
      .then(on_post_tool_use)
      .then(on_stop)
      .expect(status == Done)
  }
}
