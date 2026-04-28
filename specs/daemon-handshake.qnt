// Daemon handshake/lifecycle model.

module daemon_handshake {
  var server_running: bool
  var client_version: int
  var subscribed: bool
  var queue_depth: int
  var stopped: bool

  pure def version_ok(v: int): bool = v == 1

  action init: bool = all {
    server_running' = false,
    client_version' = 1,
    subscribed' = false,
    queue_depth' = 0,
    stopped' = false
  }

  action start: bool = all {
    server_running' = true,
    client_version' = client_version,
    subscribed' = subscribed,
    queue_depth' = queue_depth,
    stopped' = false
  }

  action bad_version: bool = all {
    server_running,
    client_version' = 0,
    server_running' = server_running,
    subscribed' = subscribed,
    queue_depth' = queue_depth,
    stopped' = stopped
  }

  action retry_supported: bool = all {
    server_running,
    not(version_ok(client_version)),
    client_version' = 1,
    server_running' = server_running,
    subscribed' = subscribed,
    queue_depth' = queue_depth,
    stopped' = stopped
  }

  action subscribe: bool = all {
    server_running,
    version_ok(client_version),
    subscribed' = true,
    server_running' = server_running,
    client_version' = client_version,
    queue_depth' = queue_depth,
    stopped' = stopped
  }

  action unsubscribe: bool = all {
    subscribed,
    subscribed' = false,
    server_running' = server_running,
    client_version' = client_version,
    queue_depth' = queue_depth,
    stopped' = stopped
  }

  action stop: bool = all {
    server_running,
    queue_depth == 0,
    server_running' = false,
    subscribed' = false,
    stopped' = true,
    client_version' = client_version,
    queue_depth' = 0
  }

  action step: bool = any {
    start,
    bad_version,
    retry_supported,
    subscribe,
    unsubscribe,
    stop
  }

  val stopped_has_no_subs: bool = stopped implies not(subscribed)
  val queue_non_negative: bool = queue_depth >= 0
  val supported_version_after_retry: bool =
    version_ok(client_version) or client_version == 0

  run mismatch_retry_subscribe = {
    init.then(start).then(bad_version).then(retry_supported).then(subscribe)
      .expect(subscribed and version_ok(client_version))
  }

  run stop_drains_subs = {
    init.then(start).then(subscribe).then(unsubscribe).then(stop)
      .expect(stopped_has_no_subs)
  }
}
