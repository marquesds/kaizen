// Cache-first read/report commands. Default path is query-only and never scans,
// indexes, pulls, or writes. Refresh/force/stale-provider may take slow path.
module read_report_perf {
  var phase: str
  var command: str
  var refresh: bool
  var force: bool
  var provider_stale: bool
  var mutated: bool

  action init: bool = all {
    phase' = "Idle",
    command' = "retro",
    refresh' = false,
    force' = false,
    provider_stale' = false,
    mutated' = false
  }

  action choose_default_read: bool = all {
    phase == "Idle",
    command' = "read_report",
    refresh' = false,
    force' = false,
    provider_stale' = false,
    mutated' = false,
    phase' = "Chosen"
  }

  action choose_refresh_read: bool = all {
    phase == "Idle",
    command' = "read_report",
    refresh' = true,
    force' = false,
    provider_stale' = false,
    mutated' = false,
    phase' = "Chosen"
  }

  action choose_force_index: bool = all {
    phase == "Idle",
    command' = "metrics_index",
    refresh' = false,
    force' = true,
    provider_stale' = false,
    mutated' = false,
    phase' = "Chosen"
  }

  action choose_stale_provider: bool = all {
    phase == "Idle",
    command' = "read_report",
    refresh' = false,
    force' = false,
    provider_stale' = true,
    mutated' = false,
    phase' = "Chosen"
  }

  action open_query: bool = all {
    phase == "Chosen",
    not(refresh),
    not(force),
    not(provider_stale),
    phase' = "OpenQuery",
    command' = command,
    refresh' = refresh,
    force' = force,
    provider_stale' = provider_stale,
    mutated' = false
  }

  action load_cached: bool = all {
    phase == "OpenQuery",
    phase' = "LoadCached",
    command' = command,
    refresh' = refresh,
    force' = force,
    provider_stale' = provider_stale,
    mutated' = mutated
  }

  action refresh_cache: bool = all {
    phase == "Chosen",
    refresh or provider_stale,
    phase' = "RefreshCache",
    command' = command,
    refresh' = refresh,
    force' = force,
    provider_stale' = provider_stale,
    mutated' = true
  }

  action index_repo: bool = all {
    phase == "Chosen",
    force,
    phase' = "IndexRepo",
    command' = command,
    refresh' = refresh,
    force' = force,
    provider_stale' = provider_stale,
    mutated' = true
  }

  action load_after_mutation: bool = all {
    phase == "RefreshCache" or phase == "IndexRepo",
    phase' = "LoadCached",
    command' = command,
    refresh' = refresh,
    force' = force,
    provider_stale' = provider_stale,
    mutated' = mutated
  }

  action render: bool = all {
    phase == "LoadCached",
    phase' = "Render",
    command' = command,
    refresh' = refresh,
    force' = force,
    provider_stale' = provider_stale,
    mutated' = mutated
  }

  action done: bool = all {
    phase == "Render",
    phase' = "Done",
    command' = command,
    refresh' = refresh,
    force' = force,
    provider_stale' = provider_stale,
    mutated' = mutated
  }

  action step: bool = any {
    choose_default_read,
    choose_refresh_read,
    choose_force_index,
    choose_stale_provider,
    open_query,
    load_cached,
    refresh_cache,
    index_repo,
    load_after_mutation,
    render,
    done
  }

  val default_read_never_mutates: bool =
    (not(refresh) and not(force) and not(provider_stale)) implies not(mutated)

  val default_read_uses_query_path: bool =
    (phase == "Done" and not(refresh) and not(force) and not(provider_stale)) implies not(mutated)

  val slow_paths_may_mutate: bool =
    mutated implies (refresh or force or provider_stale)

  run default_read_report = {
    init
      .then(choose_default_read)
      .then(open_query)
      .then(load_cached)
      .then(render)
      .then(done)
      .expect(phase == "Done" and not(mutated))
  }

  run refresh_read_report = {
    init
      .then(choose_refresh_read)
      .then(refresh_cache)
      .then(load_after_mutation)
      .then(render)
      .then(done)
      .expect(phase == "Done" and mutated)
  }
}
