# Phase 3 — Daemon Split

Single-writer daemon owns all writes. TUI / CLI / MCP become read-only
clients via Unix socket (Arrow IPC). Eliminates WAL contention; enables
push-based snapshots; opens path for Phase 4 hot tier.
Estimated effort: 2-3 sprints. Risk: high (process lifecycle, IPC).

## Scope

### 3.1 Process model

```
┌──────────────────────────────────────────────────────┐
│ kaizen daemon (long-lived, per-host or per-workspace)│
│   pid: ~/.kaizen/daemon.pid                          │
│   sock: ~/.kaizen/daemon.sock                        │
│   log:  ~/.kaizen/daemon.log                         │
└──────────────────┬───────────────────────────────────┘
   ▲              │ Arrow IPC frames
   │ ingest       │
   │ (hooks,      ▼
   │  tails,    ┌───────────────────────────────┐
   │  proxy)    │ kaizen tui / sessions / retro │
   │            │   (read-only clients)         │
   └────────────┴───────────────────────────────┘
```

Single writer means: sqlite/redb/log writes all funnel through daemon
mpsc. Clients never `INSERT`. WAL contention disappears.

### 3.2 Lifecycle

- `kaizen daemon start` — explicit foreground for debugging.
- Auto-spawn: `kaizen tui` / `kaizen sessions list` checks
  `daemon.pid`; if dead, `fork+exec` a backgrounded daemon, wait for
  socket.
- `kaizen daemon stop` — graceful: drain ingest queue, fsync, exit.
- `kaizen daemon status` — pid, uptime, queue depth, last error.
- Crash-restart via systemd unit (Linux), launchd plist (macOS),
  shipped under `packaging/`.

### 3.3 Wire protocol

Arrow IPC for bulk data (rows, snapshots). Small JSON envelope for
control (subscribe/unsubscribe/error).

```
ClientHello { proto_version: u32, client: enum { Tui, Cli, Mcp } }
ServerHello { proto_version, daemon_version, workspaces: [..] }

// requests
ListSessions { workspace, offset, limit, filter } -> Arrow batch
GetSessionDetail { id } -> Arrow batches (events, spans)
Subscribe { workspace, kinds: [SessionList, Detail{id}, Report] }
Unsubscribe { sub_id }

// pushes
Delta { sub_id, batch: Arrow IPC }   // incremental update
```

Versioned. Mismatch → server returns supported range; client retries
or errors with upgrade hint. Spec: `specs/daemon-handshake.qnt`.

### 3.4 Snapshot publisher

Daemon maintains in-memory views (session list per workspace,
report per workspace). On projector deltas, marks views dirty;
debounced (~50ms) publish-to-subscribers.

Clients hold `Arc<ArcSwap<View>>`; on push, swap pointer. Render
loop reads atomic, never blocks on IO.

### 3.5 Direct mode (no-daemon escape hatch)

`KAIZEN_DAEMON=0` or `--no-daemon` keeps Phase 0-2 behavior: client
opens SQLite directly. Used for:

- CI / smoke tests.
- Single-shot CLI calls where daemon spawn cost > query cost.
- Debug.

Mode chosen at startup; both code paths compiled. Tested in CI.

### 3.6 Auth / multi-tenant

None. Daemon is per-Unix-user. Socket has 0600 perms. No network
listener (ever — refused by config validation). Local-only.

## Acceptance criteria

| Metric | After P2 | Target P3 |
|---|---|---|
| TUI cold start (warm daemon) | ~300ms | <100ms |
| Refresh p99 (push-based) | ~50ms | <16ms |
| Concurrent writers blocking | yes (WAL) | none |
| MCP tool call latency | ~80ms | <20ms |

Integration test: `tests/spec/daemon_lifecycle.rs` — spawn, ingest,
crash, restart, replay correctness.

## Rollback

`KAIZEN_DAEMON=0` ships and stays as a supported mode. If daemon
proves flaky, default flips back; users keep Phase 2 perf.

## Risk

- Process lifecycle bugs (zombies, double-spawn, stale sockets).
  Mitigated by `fs4` advisory lock on `daemon.pid` (already a dep).
- Protocol versioning mistakes — addressed by Quint spec + frozen
  `proto_version` in CHANGELOG entries.
- Debugging harder (two processes). Daemon log + `kaizen daemon
  status` + structured `tracing` mitigate.
- Windows: Unix socket → named pipe; cross-platform IPC abstraction
  added behind trait `IpcTransport`.

## Out of scope

Storage tiering (Phase 4). Search (Phase 5). Network protocol
(explicitly forbidden, localhost-only).

## Dependencies

Requires Phase 2 (incremental projector) — daemon's value is the
single-writer invariant; without P2, daemon is just a proxy with
the same O(n²) bug.
