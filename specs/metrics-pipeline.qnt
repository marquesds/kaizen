// `kaizen metrics`: scan, index snapshot, build report, emit.
module metrics_pipeline {
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

  action ensure_index: bool = all {
    phase == "Scanned",
    phase' = "Indexed"
  }

  action build_report: bool = all {
    phase == "Indexed",
    phase' = "Reported"
  }

  action emit_output: bool = all {
    phase == "Reported",
    phase' = "Done"
  }

  action step: bool = any {
    resolve_workspace,
    load_config,
    open_store,
    scan_agents,
    ensure_index,
    build_report,
    emit_output
  }

  val index_before_report: bool =
    (phase == "Reported") implies true

  run full_run = {
    init
      .then(resolve_workspace)
      .then(load_config)
      .then(open_store)
      .then(scan_agents)
      .then(ensure_index)
      .then(build_report)
      .then(emit_output)
      .expect(phase == "Done")
  }
}
