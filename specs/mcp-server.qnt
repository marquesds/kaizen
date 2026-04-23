// kaizen MCP (stdio): tools mirror CLI; TUI and sync-daemon are special-cased.
// - Every `kaizen_tui` response is the interactive-unavailable path (stubs, not a live TUI).
// - `kaizen_sync_run` with once=false is rejected; once=true is the only flush path modeled here.
module mcp_server {
  var connected: bool
  var tuiCalls: int
  var tuiUnavailable: int
  var syncDaemonAttempts: int
  var syncDaemonRejects: int
  var syncOnceFlushes: int

  action init: bool = all {
    connected' = false,
    tuiCalls' = 0,
    tuiUnavailable' = 0,
    syncDaemonAttempts' = 0,
    syncDaemonRejects' = 0,
    syncOnceFlushes' = 0
  }

  // Client completed MCP handshake (abstract).
  action client_connect: bool = all {
    not(connected),
    connected' = true,
    tuiCalls' = tuiCalls,
    tuiUnavailable' = tuiUnavailable,
    syncDaemonAttempts' = syncDaemonAttempts,
    syncDaemonRejects' = syncDaemonRejects,
    syncOnceFlushes' = syncOnceFlushes
  }

  action tui_call: bool = all {
    connected,
    tuiCalls' = tuiCalls + 1,
    tuiUnavailable' = tuiUnavailable + 1,
    connected' = true,
    syncDaemonAttempts' = syncDaemonAttempts,
    syncDaemonRejects' = syncDaemonRejects,
    syncOnceFlushes' = syncOnceFlushes
  }

  action sync_run_daemon: bool = all {
    connected,
    syncDaemonAttempts' = syncDaemonAttempts + 1,
    syncDaemonRejects' = syncDaemonRejects + 1,
    connected' = true,
    tuiCalls' = tuiCalls,
    tuiUnavailable' = tuiUnavailable,
    syncOnceFlushes' = syncOnceFlushes
  }

  action sync_run_once: bool = all {
    connected,
    syncOnceFlushes' = syncOnceFlushes + 1,
    connected' = true,
    tuiCalls' = tuiCalls,
    tuiUnavailable' = tuiUnavailable,
    syncDaemonAttempts' = syncDaemonAttempts,
    syncDaemonRejects' = syncDaemonRejects
  }

  action step: bool = any {
    client_connect,
    tui_call,
    sync_run_daemon,
    sync_run_once
  }

  val tui_unavailable_parity: bool = tuiCalls == tuiUnavailable

  val sync_daemon_reject_parity: bool = syncDaemonAttempts == syncDaemonRejects

  val unconnected_is_quiet: bool = not(connected) implies
    tuiCalls == 0
      and tuiUnavailable == 0
      and syncDaemonAttempts == 0
      and syncDaemonRejects == 0
      and syncOnceFlushes == 0

  run tuiAndSyncEdgePath = {
    init
      .then(client_connect)
      .then(tui_call)
      .then(sync_run_daemon)
      .then(sync_run_once)
      .expect(
        all {
          tui_unavailable_parity,
          sync_daemon_reject_parity,
          connected
        }
      )
  }
}
