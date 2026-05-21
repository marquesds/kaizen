# Kaizen daemon

Phase 3 adds a local daemon so one process owns store writes.

## Commands

| Command | Purpose |
|---|---|
| `kaizen daemon start` | Run daemon in foreground for debugging |
| `kaizen daemon start --background` | Spawn daemon, wait until ready, print pid/socket/log, exit |
| `kaizen daemon status` | Print `status: running` plus pid/uptime/queue/error/capture, or `status: stopped` plus socket path |
| `kaizen daemon stop` | Request graceful daemon shutdown |
| `--no-daemon` or `KAIZEN_DAEMON=0` | Use direct SQLite mode |

Runtime files live under `$KAIZEN_HOME` or `~/.kaizen`:

| File | Purpose |
|---|---|
| `daemon.pid` | Locked pid file |
| `daemon.sock` | Local Unix socket, mode `0600` |
| `daemon.log` | Background daemon stdout/stderr |

## Protocol

Clients send length-prefixed JSON control frames with `proto_version = 1`.
Bulk query responses are shaped so Arrow IPC batches can replace JSON payloads
without changing lifecycle commands. Unsupported versions return supported min
and max.

Current daemon-backed paths include hook ingest, `sessions list`, init capture,
daemon-owned transcript scanning, and observe/proxy session setup. Direct mode
remains compiled and supported for CI, smoke tests, and debugging.

`kaizen init` calls the daemon with `EnsureWorkspaceCapture`. That starts a
workspace scanner loop and records capture health for `daemon status`. `kaizen
init --deep` also asks for provider proxy endpoints; unsupported agent config
rewrites stay fail-open and are reported as partial deep capture.
