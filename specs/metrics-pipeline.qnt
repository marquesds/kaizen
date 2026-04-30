// `kaizen metrics`: scan, index snapshot, build report, emit.
module metrics_pipeline {
  var phase: str
  var workspace_kind: str
  var index_succeeded: bool

  action init: bool = all {
    phase' = "Idle",
    workspace_kind' = "",
    index_succeeded' = false
  }

  action resolve_git_workspace: bool = all {
    phase == "Idle",
    phase' = "Resolved",
    workspace_kind' = "git",
    index_succeeded' = false
  }

  action resolve_plain_workspace: bool = all {
    phase == "Idle",
    phase' = "Resolved",
    workspace_kind' = "plain",
    index_succeeded' = false
  }

  action load_config: bool = all {
    phase == "Resolved",
    phase' = "ConfigLoaded",
    workspace_kind' = workspace_kind,
    index_succeeded' = index_succeeded
  }

  action open_store: bool = all {
    phase == "ConfigLoaded",
    phase' = "StoreOpen",
    workspace_kind' = workspace_kind,
    index_succeeded' = index_succeeded
  }

  action scan_agents: bool = all {
    phase == "StoreOpen",
    phase' = "Scanned",
    workspace_kind' = workspace_kind,
    index_succeeded' = index_succeeded
  }

  action ensure_index: bool = all {
    phase == "Scanned",
    workspace_kind == "git" or workspace_kind == "plain",
    phase' = "Indexed",
    workspace_kind' = workspace_kind,
    index_succeeded' = true
  }

  action build_report: bool = all {
    phase == "Indexed",
    index_succeeded,
    phase' = "Reported",
    workspace_kind' = workspace_kind,
    index_succeeded' = index_succeeded
  }

  action emit_output: bool = all {
    phase == "Reported",
    phase' = "Done",
    workspace_kind' = workspace_kind,
    index_succeeded' = index_succeeded
  }

  action step: bool = any {
    resolve_git_workspace,
    resolve_plain_workspace,
    load_config,
    open_store,
    scan_agents,
    ensure_index,
    build_report,
    emit_output
  }

  val index_before_report: bool =
    (phase == "Reported") implies index_succeeded

  val index_supports_git_and_plain: bool =
    index_succeeded implies (workspace_kind == "git" or workspace_kind == "plain")

  run git_full_run = {
    init
      .then(resolve_git_workspace)
      .then(load_config)
      .then(open_store)
      .then(scan_agents)
      .then(ensure_index)
      .then(build_report)
      .then(emit_output)
      .expect(phase == "Done")
  }

  run plain_full_run = {
    init
      .then(resolve_plain_workspace)
      .then(load_config)
      .then(open_store)
      .then(scan_agents)
      .then(ensure_index)
      .then(build_report)
      .then(emit_output)
      .expect(phase == "Done" and index_supports_git_and_plain)
  }
}
