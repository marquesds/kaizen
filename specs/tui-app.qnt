// TUI: boot, event-driven refresh, background report publish, ordered shutdown.
module tui_app {
  var phase: str
  var dirty: bool
  var reportDirty: bool
  var reportComputing: bool
  var reportReady: bool
  var walEvents: int
  var manualRefreshes: int
  var refreshes: int
  var reportPublishes: int

  action init: bool = all {
    phase' = "Boot",
    dirty' = false,
    reportDirty' = true,
    reportComputing' = false,
    reportReady' = false,
    walEvents' = 0,
    manualRefreshes' = 0,
    refreshes' = 0,
    reportPublishes' = 0
  }

  action become_ready: bool = all {
    phase == "Boot",
    phase' = "Interactive",
    dirty' = dirty,
    reportDirty' = reportDirty,
    reportComputing' = reportComputing,
    reportReady' = reportReady,
    walEvents' = walEvents,
    manualRefreshes' = manualRefreshes,
    refreshes' = refreshes,
    reportPublishes' = reportPublishes
  }

  // Any WAL write marks UI dirty; bursts coalesce because `dirty` stays boolean.
  action wal_change: bool = all {
    phase == "Interactive",
    phase' = phase,
    dirty' = true,
    reportDirty' = reportDirty,
    reportComputing' = reportComputing,
    reportReady' = reportReady,
    walEvents' = walEvents + 1,
    manualRefreshes' = manualRefreshes,
    refreshes' = refreshes,
    reportPublishes' = reportPublishes
  }

  // One refresh services one or more WAL writes and schedules report recompute.
  action coalesced_refresh: bool = all {
    phase == "Interactive",
    dirty,
    phase' = phase,
    dirty' = false,
    reportDirty' = true,
    reportComputing' = reportComputing,
    reportReady' = reportReady,
    walEvents' = walEvents,
    manualRefreshes' = manualRefreshes,
    refreshes' = refreshes + 1,
    reportPublishes' = reportPublishes
  }

  action manual_refresh: bool = all {
    phase == "Interactive",
    phase' = phase,
    dirty' = false,
    reportDirty' = true,
    reportComputing' = reportComputing,
    reportReady' = reportReady,
    walEvents' = walEvents,
    manualRefreshes' = manualRefreshes + 1,
    refreshes' = refreshes + 1,
    reportPublishes' = reportPublishes
  }

  action report_recompute: bool = all {
    phase == "Interactive",
    reportDirty,
    not(reportComputing),
    phase' = phase,
    dirty' = dirty,
    reportDirty' = false,
    reportComputing' = true,
    reportReady' = reportReady,
    walEvents' = walEvents,
    manualRefreshes' = manualRefreshes,
    refreshes' = refreshes,
    reportPublishes' = reportPublishes
  }

  action report_publish: bool = all {
    phase == "Interactive",
    reportComputing,
    phase' = phase,
    dirty' = dirty,
    reportDirty' = reportDirty,
    reportComputing' = false,
    reportReady' = true,
    walEvents' = walEvents,
    manualRefreshes' = manualRefreshes,
    refreshes' = refreshes,
    reportPublishes' = reportPublishes + 1
  }

  action request_quit: bool = all {
    phase == "Interactive",
    phase' = "Draining",
    dirty' = dirty,
    reportDirty' = reportDirty,
    reportComputing' = reportComputing,
    reportReady' = reportReady,
    walEvents' = walEvents,
    manualRefreshes' = manualRefreshes,
    refreshes' = refreshes,
    reportPublishes' = reportPublishes
  }

  action finish_shutdown: bool = all {
    phase == "Draining",
    phase' = "Exited",
    dirty' = dirty,
    reportDirty' = reportDirty,
    reportComputing' = reportComputing,
    reportReady' = reportReady,
    walEvents' = walEvents,
    manualRefreshes' = manualRefreshes,
    refreshes' = refreshes,
    reportPublishes' = reportPublishes
  }

  action step: bool = any {
    become_ready,
    wal_change,
    coalesced_refresh,
    manual_refresh,
    report_recompute,
    report_publish,
    request_quit,
    finish_shutdown
  }

  val refreshes_have_signals: bool =
    refreshes <= walEvents + manualRefreshes

  val no_refresh_needed_without_dirty: bool =
    (not(dirty) and manualRefreshes == 0) implies refreshes == 0

  val report_publish_requires_work: bool =
    reportPublishes > 0 implies reportReady

  val exited_after_draining: bool =
    phase == "Exited" implies not(phase == "Interactive")

  run normal_exit = {
    init
      .then(become_ready)
      .then(request_quit)
      .then(finish_shutdown)
      .expect(phase == "Exited")
  }

  run wal_burst_coalesces = {
    init
      .then(become_ready)
      .then(wal_change)
      .then(wal_change)
      .then(coalesced_refresh)
      .expect(refreshes == 1 and walEvents == 2 and not(dirty))
  }

  run report_background_publish = {
    init
      .then(become_ready)
      .then(manual_refresh)
      .then(report_recompute)
      .then(report_publish)
      .expect(phase == "Interactive" and reportReady and reportPublishes == 1)
  }
}
