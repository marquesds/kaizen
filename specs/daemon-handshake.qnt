// Daemon handshake/lifecycle model.

module daemon_handshake {
  var server_running: bool
  var client_version: int
  var subscribed: bool
  var queue_depth: int
  var stopped: bool
  var background_returned: bool
  var status_result: str

  pure def version_ok(v: int): bool = v == 1

  action init: bool = all {
    server_running' = false,
    client_version' = 1,
    subscribed' = false,
    queue_depth' = 0,
    stopped' = false,
    background_returned' = false,
    status_result' = "none"
  }

  action start: bool = all {
    server_running' = true,
    client_version' = client_version,
    subscribed' = subscribed,
    queue_depth' = queue_depth,
    stopped' = false,
    background_returned' = background_returned,
    status_result' = status_result
  }

  // `kaizen daemon start --background` returns only after the child server is ready.
  action start_background: bool = all {
    server_running' = true,
    client_version' = client_version,
    subscribed' = subscribed,
    queue_depth' = queue_depth,
    stopped' = false,
    background_returned' = true,
    status_result' = status_result
  }

  action bad_version: bool = all {
    server_running,
    client_version' = 0,
    server_running' = server_running,
    subscribed' = subscribed,
    queue_depth' = queue_depth,
    stopped' = stopped,
    background_returned' = background_returned,
    status_result' = status_result
  }

  action retry_supported: bool = all {
    server_running,
    not(version_ok(client_version)),
    client_version' = 1,
    server_running' = server_running,
    subscribed' = subscribed,
    queue_depth' = queue_depth,
    stopped' = stopped,
    background_returned' = background_returned,
    status_result' = status_result
  }

  action subscribe: bool = all {
    server_running,
    version_ok(client_version),
    subscribed' = true,
    server_running' = server_running,
    client_version' = client_version,
    queue_depth' = queue_depth,
    stopped' = stopped,
    background_returned' = background_returned,
    status_result' = status_result
  }

  action unsubscribe: bool = all {
    subscribed,
    subscribed' = false,
    server_running' = server_running,
    client_version' = client_version,
    queue_depth' = queue_depth,
    stopped' = stopped,
    background_returned' = background_returned,
    status_result' = status_result
  }

  action stop: bool = all {
    server_running,
    queue_depth == 0,
    server_running' = false,
    subscribed' = false,
    stopped' = true,
    client_version' = client_version,
    queue_depth' = 0,
    background_returned' = false,
    status_result' = status_result
  }

  action status_running: bool = all {
    server_running,
    version_ok(client_version),
    server_running' = server_running,
    client_version' = client_version,
    subscribed' = subscribed,
    queue_depth' = queue_depth,
    stopped' = stopped,
    background_returned' = background_returned,
    status_result' = "running"
  }

  action status_stopped: bool = all {
    not(server_running),
    server_running' = server_running,
    client_version' = client_version,
    subscribed' = subscribed,
    queue_depth' = queue_depth,
    stopped' = stopped,
    background_returned' = background_returned,
    status_result' = "stopped"
  }

  action status_protocol_error: bool = all {
    server_running,
    not(version_ok(client_version)),
    server_running' = server_running,
    client_version' = client_version,
    subscribed' = subscribed,
    queue_depth' = queue_depth,
    stopped' = stopped,
    background_returned' = background_returned,
    status_result' = "error"
  }

  action step: bool = any {
    start,
    start_background,
    bad_version,
    retry_supported,
    subscribe,
    unsubscribe,
    stop,
    status_running,
    status_stopped,
    status_protocol_error
  }

  val stopped_has_no_subs: bool = stopped implies not(subscribed)
  val queue_non_negative: bool = queue_depth >= 0
  val background_return_implies_ready: bool =
    background_returned implies server_running
  val supported_version_after_retry: bool =
    version_ok(client_version) or client_version == 0
  val stopped_status_requires_no_server: bool =
    (status_result == "stopped") implies not(server_running)
  val running_status_requires_server: bool =
    (status_result == "running") implies server_running

  run mismatch_retry_subscribe = {
    init.then(start).then(bad_version).then(retry_supported).then(subscribe)
      .expect(subscribed and version_ok(client_version))
  }

  run stop_drains_subs = {
    init.then(start).then(subscribe).then(unsubscribe).then(stop)
      .expect(stopped_has_no_subs)
  }

  run background_start_ready = {
    init.then(start_background)
      .expect(background_return_implies_ready)
  }

  run status_after_stop_is_stopped = {
    init.then(start_background).then(stop).then(status_stopped)
      .expect(status_result == "stopped")
  }

  run bad_version_status_errors = {
    init.then(start).then(bad_version).then(status_protocol_error)
      .expect(status_result == "error")
  }
}
