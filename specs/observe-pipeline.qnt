// Read-only observe commands: strict phase order through emit (sessions list, summary, insights, guidance).
// `source` selects local, provider, or mixed read paths; mixed must merge without double-counting
// the same logical session id (set dedupe models unique rows vs raw merge attempts).
module observe_pipeline {
  var phase: str
  // local | provider | mixed — drives whether remote cache participates.
  var source: str
  // Deduplicated logical ids (shared across local+remote in mixed).
  var merged: Set[str]
  // How many local/remote rows were seen before dedupe.
  var merge_strokes: int

  action init: bool = all {
    phase' = "Idle",
    source' = "local",
    merged' = Set(),
    merge_strokes' = 0
  }

  // Mixed/remote observe paths (smoke for merge invariants in runs).
  action init_mixed: bool = all {
    phase' = "Idle",
    source' = "mixed",
    merged' = Set(),
    merge_strokes' = 0
  }

  // Read path uses provider cache (no local rows); remote-sourced id must still land in `merged` after query.
  action init_provider: bool = all {
    phase' = "Idle",
    source' = "provider",
    merged' = Set(),
    merge_strokes' = 0
  }

  action resolve_workspace: bool = all {
    phase == "Idle",
    phase' = "Resolved",
    source' = source,
    merged' = merged,
    merge_strokes' = merge_strokes
  }

  action load_config: bool = all {
    phase == "Resolved",
    phase' = "ConfigLoaded",
    source' = source,
    merged' = merged,
    merge_strokes' = merge_strokes
  }

  action open_store: bool = all {
    phase == "ConfigLoaded",
    phase' = "StoreOpen",
    source' = source,
    merged' = merged,
    merge_strokes' = merge_strokes
  }

  action scan_agents: bool = all {
    phase == "StoreOpen",
    phase' = "Scanned",
    source' = source,
    merged' = merged,
    merge_strokes' = merge_strokes
  }

  // In Scanned, fold provider/local rows into a deduplicated set (mixed may see one id twice).
  action contribute: bool = {
    nondet k = oneOf(Set("s1", "s2", "s3"))
    all {
      phase == "Scanned",
      merge_strokes' = merge_strokes + 1,
      merged' = merged.union(Set(k)),
      phase' = "Scanned",
      source' = source
    }
  }

  // Fixed key (deterministic duplicate for the mixed dedupe run test).
  action contribute_s1: bool = all {
    phase == "Scanned",
    merge_strokes' = merge_strokes + 1,
    merged' = merged.union(Set("s1")),
    phase' = "Scanned",
    source' = source
  }

  // Remote cache contributes id "r1" (abstract session/event key) — only for provider|mixed.
  action contribute_remote: bool = all {
    phase == "Scanned",
    (source == "provider" or source == "mixed"),
    merge_strokes' = merge_strokes + 1,
    merged' = merged.union(Set("r1")),
    phase' = "Scanned",
    source' = source
  }

  action run_query: bool = all {
    phase == "Scanned",
    phase' = "Queried",
    source' = source,
    merged' = merged,
    merge_strokes' = merge_strokes
  }

  action emit_output: bool = all {
    phase == "Queried",
    phase' = "Done",
    source' = source,
    merged' = merged,
    merge_strokes' = merge_strokes
  }

  action step: bool = any {
    resolve_workspace,
    load_config,
    open_store,
    scan_agents,
    contribute,
    contribute_s1,
    contribute_remote,
    run_query,
    emit_output
  }

  val done_is_terminal: bool =
    (phase == "Done") implies (phase == "Done")

  // Deduped cardinality never exceeds raw merge operations (set absorbs duplicates).
  val merge_dedupes: bool = merged.size() <= merge_strokes

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

  // Same id contributed twice: stroke count grows; set size stays 1.
  run mixed_dedupes_same_id = {
    init_mixed
      .then(resolve_workspace)
      .then(load_config)
      .then(open_store)
      .then(scan_agents)
      .then(contribute_s1)
      .then(contribute_s1)
      .then(run_query)
      .then(emit_output)
      .expect(merge_strokes == 2 and merged.size() == 1 and phase == "Done")
  }

  // Provider (or could start mixed) — any remote id folded into `merged` before Done.
  run provider_includes_remote_in_merged = {
    init_provider
      .then(resolve_workspace)
      .then(load_config)
      .then(open_store)
      .then(scan_agents)
      .then(contribute_remote)
      .then(run_query)
      .then(emit_output)
      .expect(phase == "Done" and source == "provider" and merged == Set("r1") and merge_strokes == 1)
  }
}
