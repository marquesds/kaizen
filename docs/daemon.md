# Kaizen daemon

Kaizen can run a local daemon so one process owns store writes.

## Commands

| Command | Purpose |
|---|---|
| `kaizen open` | Start the daemon when needed and open the local Web app |
| `kaizen open --no-browser` | Start the daemon and print the authenticated URL |
| `kaizen daemon start` | Run daemon in foreground for debugging |
| `kaizen daemon start --background` | Spawn daemon, wait until ready, print pid/socket/log/web, exit |
| `kaizen daemon status` | Print `status: running` plus pid/uptime/queue/error/capture/web, or `status: stopped` plus socket path |
| `kaizen daemon stop` | Request graceful daemon shutdown |
| `--no-daemon` or `KAIZEN_DAEMON=0` | Use direct SQLite mode |

Runtime files live under `$KAIZEN_HOME` or `~/.kaizen`:

| File | Purpose |
|---|---|
| `daemon.pid` | Locked pid file |
| `daemon.sock` | Local Unix socket, mode `0600` |
| `daemon.log` | Background daemon stdout/stderr |
| `web_token.hex` | Restart-stable Web auth token, mode `0600` on Unix |

## Web app

Daemon mode also starts a loopback web app. The default bind address is
`127.0.0.1:7878`; if that port is busy, Kaizen falls back to another loopback
port. `kaizen open` is the normal entrypoint. `kaizen daemon start
--background` and `kaizen daemon status` also print the app URL with its session
token:

```text
web: http://127.0.0.1:7878/?token=<token>
```

The token remains stable across daemon restarts so an open tab can reconnect.
Malformed token files fail closed instead of silently rotating credentials.
Static HTML, CSS, and JavaScript load over HTTP. All data and actions use the
authenticated WebSocket at `/ws?token=<token>`. Web is an Observe-only surface:
its tool registry contains only `kaizen_sessions_list`, which discovers observed
projects through the shared Rust handler. Session reports and detail refreshes
use the Web snapshot protocol. Other CLI and MCP workflows are not advertised or
callable through Web.

## Protocol

Clients send length-prefixed JSON control frames with `proto_version = 1`.
Control and query payloads use JSON. Unsupported versions return supported
minimum and maximum versions.

Current daemon-backed paths include hook ingest, `sessions list`, init capture,
daemon-owned transcript scanning, and observe/proxy session setup. Direct mode
remains compiled and supported for CI, smoke tests, and debugging.

`kaizen init` calls the daemon with `EnsureWorkspaceCapture`. That starts a
workspace scanner loop and records capture health for `daemon status`. `kaizen
init --deep` also asks for provider proxy endpoints; unsupported agent config
rewrites stay fail-open and are reported as partial deep capture.
