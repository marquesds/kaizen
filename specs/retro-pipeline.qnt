// Retro pipeline: single-flight lock while non-Idle; Writing implies lock held.
// For source ∈ { provider, mixed }, an optional RemotePull runs before load when
// the cache is stale (TTL miss) or the user asked for --refresh. Atomic report
// write (Writing→Idle) is unchanged; pull stays under the same lock.
module retro_pipeline {
  var phase: str
  var lockHeld: bool
  // Inputs data source: local SQLite only, provider cache only, or merged.
  var source: str
  // When false, a remote pull is required before load (for provider/mixed).
  var cache_fresh: bool
  // User- or policy-driven force refresh.
  var wants_refresh: bool

  action init: bool = all {
    phase' = "Idle",
    lockHeld' = false,
    source' = "local",
    cache_fresh' = true,
    wants_refresh' = false
  }

  // Workspaces that will read from the provider cache.
  action init_with_remote_source: bool = all {
    phase' = "Idle",
    lockHeld' = false,
    source' = "provider",
    cache_fresh' = true,
    wants_refresh' = false
  }

  // Stale cache: provider/mixed will require RemotePull before load.
  action init_stale_provider: bool = all {
    phase' = "Idle",
    lockHeld' = false,
    source' = "provider",
    cache_fresh' = false,
    wants_refresh' = false
  }

  // load from local store only: straight to Loading.
  action acquire: bool = all {
    phase == "Idle",
    not(lockHeld),
    source == "local",
    lockHeld' = true,
    phase' = "Loading",
    source' = source,
    cache_fresh' = cache_fresh,
    wants_refresh' = wants_refresh
  }

  // provider/mixed, cache is fresh and not forcing refresh: skip pull.
  action acquire_from_cache: bool = all {
    phase == "Idle",
    not(lockHeld),
    (source == "provider" or source == "mixed"),
    cache_fresh,
    not(wants_refresh),
    lockHeld' = true,
    phase' = "Loading",
    source' = source,
    cache_fresh' = cache_fresh,
    wants_refresh' = wants_refresh
  }

  // provider/mixed, TTL miss or --refresh: pull first under the same single-flight lock.
  action acquire_start_pull: bool = all {
    phase == "Idle",
    not(lockHeld),
    (source == "provider" or source == "mixed"),
    (not(cache_fresh) or wants_refresh),
    lockHeld' = true,
    phase' = "RemotePull",
    source' = source,
    cache_fresh' = cache_fresh,
    wants_refresh' = wants_refresh
  }

  action pull_done: bool = all {
    phase == "RemotePull",
    lockHeld' = true,
    phase' = "Loading",
    source' = source,
    cache_fresh' = true,
    wants_refresh' = false
  }

  action load_done: bool = all {
    phase == "Loading",
    lockHeld' = lockHeld,
    phase' = "Computing",
    source' = source,
    cache_fresh' = cache_fresh,
    wants_refresh' = wants_refresh
  }

  action compute_done: bool = all {
    phase == "Computing",
    lockHeld' = lockHeld,
    phase' = "Ranking",
    source' = source,
    cache_fresh' = cache_fresh,
    wants_refresh' = wants_refresh
  }

  action rank_done: bool = all {
    phase == "Ranking",
    lockHeld' = lockHeld,
    phase' = "Writing",
    source' = source,
    cache_fresh' = cache_fresh,
    wants_refresh' = wants_refresh
  }

  action write_done: bool = all {
    phase == "Writing",
    lockHeld' = false,
    phase' = "Idle",
    source' = source,
    cache_fresh' = cache_fresh,
    wants_refresh' = wants_refresh
  }

  action step: bool = any {
    acquire,
    acquire_from_cache,
    acquire_start_pull,
    pull_done,
    load_done,
    compute_done,
    rank_done,
    write_done
  }

  val writing_implies_lock: bool =
    (phase == "Writing") implies lockHeld

  val idle_unlocked: bool =
    (phase == "Idle") implies not(lockHeld)

  val pull_holds_lock: bool =
    (phase == "RemotePull") implies lockHeld

  run linear_with_pull = {
    init_stale_provider
      .then(acquire_start_pull)
      .then(pull_done)
      .then(load_done)
      .then(compute_done)
      .then(rank_done)
      .then(write_done)
      .expect(phase == "Idle" and not(lockHeld) and cache_fresh)
  }
}
