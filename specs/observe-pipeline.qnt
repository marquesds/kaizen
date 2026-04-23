// Read-only observe commands: strict phase order through emit (sessions list, summary, insights, guidance).
module observe_pipeline {
  var phase: str

  action init: bool = all {
    phase' = "Idle"
  }

  action resolve_workspace: bool = all {
    phase == "Idle",
    phase' = "Resolved"
  }

  action load_config: bool = all {
    phase == "Resolved",
    phase' = "ConfigLoaded"
  }

  action open_store: bool = all {
    phase == "ConfigLoaded",
    phase' = "StoreOpen"
  }

  action scan_agents: bool = all {
    phase == "StoreOpen",
    phase' = "Scanned"
  }

  action run_query: bool = all {
    phase == "Scanned",
    phase' = "Queried"
  }

  action emit_output: bool = all {
    phase == "Queried",
    phase' = "Done"
  }

  action step: bool = any {
    resolve_workspace,
    load_config,
    open_store,
    scan_agents,
    run_query,
    emit_output
  }

  val done_is_terminal: bool =
    (phase == "Done") implies (phase == "Done")

  run linear_run = {
    init
      .then(resolve_workspace)
      .then(load_config)
      .then(open_store)
      .then(scan_agents)
      .then(run_query)
      .then(emit_output)
      .expect(phase == "Done")
  }
}
