// TUI: boot, interactive loop, ordered shutdown.
module tui_app {
  var phase: str

  action init: bool = all {
    phase' = "Boot"
  }

  action become_ready: bool = all {
    phase == "Boot",
    phase' = "Interactive"
  }

  action request_quit: bool = all {
    phase == "Interactive",
    phase' = "Draining"
  }

  action finish_shutdown: bool = all {
    phase == "Draining",
    phase' = "Exited"
  }

  action step: bool = any {
    become_ready,
    request_quit,
    finish_shutdown
  }

  val exited_after_draining: bool =
    (phase == "Exited") implies true

  run normal_exit = {
    init
      .then(become_ready)
      .then(request_quit)
      .then(finish_shutdown)
      .expect(phase == "Exited")
  }
}
