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
  var totalSessions: int
  var cursor: int
  var fetchedStart: int
  var fetchedEnd: int
  var pageLoading: bool
  var loadRequests: int

  action init: bool = all {
    phase' = "Boot",
    dirty' = false,
    reportDirty' = true,
    reportComputing' = false,
    reportReady' = false,
    walEvents' = 0,
    manualRefreshes' = 0,
    refreshes' = 0,
    reportPublishes' = 0,
    totalSessions' = 0,
    cursor' = 0,
    fetchedStart' = 0,
    fetchedEnd' = 0,
    pageLoading' = false,
    loadRequests' = 0
  }

  action keep_window: bool = all {
    totalSessions' = totalSessions,
    cursor' = cursor,
    fetchedStart' = fetchedStart,
    fetchedEnd' = fetchedEnd,
    pageLoading' = pageLoading,
    loadRequests' = loadRequests
  }

  action keep_lifecycle: bool = all {
    phase' = phase,
    dirty' = dirty,
    reportDirty' = reportDirty,
    reportComputing' = reportComputing,
    reportReady' = reportReady,
    walEvents' = walEvents,
    manualRefreshes' = manualRefreshes,
    refreshes' = refreshes,
    reportPublishes' = reportPublishes
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
    reportPublishes' = reportPublishes,
    keep_window
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
    reportPublishes' = reportPublishes,
    keep_window
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
    reportPublishes' = reportPublishes,
    keep_window
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
    reportPublishes' = reportPublishes,
    keep_window
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
    reportPublishes' = reportPublishes,
    keep_window
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
    reportPublishes' = reportPublishes + 1,
    keep_window
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
    reportPublishes' = reportPublishes,
    keep_window
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
    reportPublishes' = reportPublishes,
    keep_window
  }

  action page_load_request: bool = all {
    phase == "Interactive",
    not(pageLoading),
    keep_lifecycle,
    totalSessions' = totalSessions,
    cursor' = cursor,
    fetchedStart' = fetchedStart,
    fetchedEnd' = fetchedEnd,
    pageLoading' = true,
    loadRequests' = loadRequests + 1
  }

  action page_load_complete: bool = all {
    phase == "Interactive",
    pageLoading,
    keep_lifecycle,
    totalSessions' = 20,
    cursor' = cursor,
    fetchedStart' = 0,
    fetchedEnd' = 20,
    pageLoading' = false,
    loadRequests' = loadRequests
  }

  action scroll_down: bool = all {
    phase == "Interactive",
    totalSessions > 0,
    cursor + 1 < totalSessions,
    cursor + 1 < fetchedEnd,
    keep_lifecycle,
    totalSessions' = totalSessions,
    cursor' = cursor + 1,
    fetchedStart' = fetchedStart,
    fetchedEnd' = fetchedEnd,
    pageLoading' = pageLoading,
    loadRequests' = loadRequests
  }

  action filter_reset: bool = all {
    phase == "Interactive",
    keep_lifecycle,
    totalSessions' = 0,
    cursor' = 0,
    fetchedStart' = 0,
    fetchedEnd' = 0,
    pageLoading' = false,
    loadRequests' = loadRequests
  }

  action step: bool = any {
    become_ready,
    wal_change,
    coalesced_refresh,
    manual_refresh,
    report_recompute,
    report_publish,
    page_load_request,
    page_load_complete,
    scroll_down,
    filter_reset,
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

  val cursor_always_fetched: bool =
    totalSessions == 0 or (fetchedStart <= cursor and cursor < fetchedEnd)

  val fetched_range_ordered: bool =
    fetchedStart <= fetchedEnd

  val no_double_load: bool =
    pageLoading implies loadRequests > 0

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

  run window_load_then_scroll = {
    init
      .then(become_ready)
      .then(page_load_request)
      .then(page_load_complete)
      .then(scroll_down)
      .expect(cursor == 1 and cursor_always_fetched)
  }

  run filter_clears_window = {
    init
      .then(become_ready)
      .then(page_load_request)
      .then(page_load_complete)
      .then(filter_reset)
      .expect(totalSessions == 0 and cursor == 0 and not(pageLoading))
  }
}
