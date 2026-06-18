export function createTransport(options) {
  let socket = null;
  let everOpened = false;
  let reconnectTimer = 0;
  let reconnectMs = 1_000;

  function connect() {
    clearTimeout(reconnectTimer);
    socket = new WebSocket(options.url());
    socket.addEventListener("open", opened);
    socket.addEventListener("close", closed);
    socket.addEventListener("message", event => options.onMessage(event.data));
  }

  function opened() {
    everOpened = true;
    reconnectMs = 1_000;
    options.onOpen();
  }

  function closed() {
    if (!everOpened) return options.onAuthFailure();
    options.onDisconnect();
    reconnectTimer = setTimeout(connect, reconnectMs);
    reconnectMs = Math.min(reconnectMs * 2, 10_000);
  }

  function send(value) {
    if (!isOpen()) return false;
    socket.send(JSON.stringify(value));
    return true;
  }

  function isOpen() {
    return socket?.readyState === WebSocket.OPEN;
  }

  return { connect, isOpen, send };
}
