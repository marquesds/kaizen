// `kaizen metrics`: default report reads cached snapshot. Explicit refresh scans;
// explicit force index rebuilds repo snapshot.
module metrics_pipeline {
  var phase: str
  var workspace_kind: str
  var index_succeeded: bool
  var refresh: bool
  var force: bool

  action init: bool = all {
    phase' = "Idle",
    workspace_kind' = "",
    index_succeeded' = false,
    refresh' = false,
    force' = false
  }

  action resolve_git_workspace: bool = all {
    phase == "Idle",
    phase' = "Resolved",
    workspace_kind' = "git",
    index_succeeded' = false,
    refresh' = false,
    force' = false
  }

  action resolve_plain_workspace: bool = all {
    phase == "Idle",
    phase' = "Resolved",
    workspace_kind' = "plain",
    index_succeeded' = false,
    refresh' = false,
    force' = false
  }

  action resolve_force_index: bool = all {
    phase == "Idle",
    phase' = "Resolved",
    workspace_kind' = "git",
    index_succeeded' = false,
    refresh' = false,
    force' = true
  }

  action resolve_refresh: bool = all {
    phase == "Idle",
    phase' = "Resolved",
    workspace_kind' = "git",
    index_succeeded' = false,
    refresh' = true,
    force' = false
  }

  action load_config: bool = all {
    phase == "Resolved",
    phase' = "ConfigLoaded",
    workspace_kind' = workspace_kind,
    index_succeeded' = index_succeeded,
    refresh' = refresh,
    force' = force
  }

  action open_store: bool = all {
    phase == "ConfigLoaded",
    phase' = "StoreOpen",
    workspace_kind' = workspace_kind,
    index_succeeded' = index_succeeded,
    refresh' = refresh,
    force' = force
  }

  action scan_agents: bool = all {
    phase == "StoreOpen",
    refresh,
    phase' = "Scanned",
    workspace_kind' = workspace_kind,
    index_succeeded' = index_succeeded,
    refresh' = refresh,
    force' = force
  }

  action ensure_index: bool = all {
    phase == "StoreOpen" or phase == "Scanned",
    force,
    workspace_kind == "git" or workspace_kind == "plain",
    phase' = "Indexed",
    workspace_kind' = workspace_kind,
    index_succeeded' = true,
    refresh' = refresh,
    force' = force
  }

  action build_report: bool = all {
    phase == "StoreOpen" or phase == "Scanned" or phase == "Indexed",
    phase' = "Reported",
    workspace_kind' = workspace_kind,
    index_succeeded' = index_succeeded,
    refresh' = refresh,
    force' = force
  }

  action emit_output: bool = all {
    phase == "Reported",
    phase' = "Done",
    workspace_kind' = workspace_kind,
    index_succeeded' = index_succeeded,
    refresh' = refresh,
    force' = force
  }

  action step: bool = all {
    phase' = "Idle",
    workspace_kind' = "",
    index_succeeded' = false,
    refresh' = false,
    force' = false
  }

  val default_report_does_not_require_index: bool =
    (phase == "Reported" and not(force)) implies not(index_succeeded)

  val index_supports_git_and_plain: bool =
    index_succeeded implies (workspace_kind == "git" or workspace_kind == "plain")

  run git_full_run = {
    init
      .then(resolve_git_workspace)
      .then(load_config)
      .then(open_store)
      .then(build_report)
      .then(emit_output)
      .expect(phase == "Done" and not(index_succeeded))
  }

  run plain_full_run = {
    init
      .then(resolve_plain_workspace)
      .then(load_config)
      .then(open_store)
      .then(build_report)
      .then(emit_output)
      .expect(phase == "Done" and not(index_succeeded))
  }

  run force_index_run = {
    init
      .then(resolve_force_index)
      .then(load_config)
      .then(open_store)
      .then(ensure_index)
      .then(build_report)
      .then(emit_output)
      .expect(phase == "Done" and index_supports_git_and_plain)
  }
}
