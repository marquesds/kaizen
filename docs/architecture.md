# Architecture

## Module Graph

- `collect` parses agent transcripts + hooks into raw `Event`s.
- `daemon` owns local daemon lifecycle and length-prefixed IPC for supported
  client paths.
- `store` owns SQLite raw tables, the incremental event projector, and derived
  indexes (`tool_spans`, `file_facts`, `repo_edges`).
- `metrics` builds commit-pinned repo snapshots and Ladybug sidecar.
- `guidance` scores skill/rule artifacts and models proposal candidates.
- `retro` consumes raw telemetry + smart metrics for heuristic bets.
- `sync` ships redacted events, tool spans, and repo snapshots.
- `proxy` forwards LLM HTTP traffic and appends `EventSource::Proxy` rows (see
  [llm-proxy.md](llm-proxy.md)).
- `telemetry` fans out the same redacted batches to optional exporters
  (PostHog, Datadog, OTLP) when configured.
- `mcp` is the stdio MCP server; tools delegate to the same `shell` commands as
  the CLI (see [mcp.md](mcp.md)).
- `web` is the daemon-served loopback UI. It serves embedded assets and routes
  authenticated WebSocket tool calls through the same MCP-backed Rust handlers.
- `bin_kaizen` owns binary-only CLI parsing and dispatch. It keeps the runtime
  entrypoint thin while routing commands to the same `shell`, `mcp`, `daemon`,
  and `ui` surfaces.

## Data Flow

1. Transcript, hook, or (optional) proxy ingest → `events`.
2. Event append applies projector deltas for `files_touched`, `skills_used`,
   `rules_used`, and closed/orphaned `tool_spans`.
3. Metrics index scans git + source tree → `repo_snapshots`,
   `file_facts`, `repo_edges`, Ladybug sidecar.
4. Daemon-backed clients use the local socket; direct mode opens SQLite in-process.
5. Guidance scorecards join artifact inventory with evals, outcomes, feedback,
   costs, and prompt fingerprints.
6. CLI / TUI / web / retro read shared smart-metric report builder.
7. Sync flushes redacted outbox rows by kind:
   `events`, `tool_spans`, `repo_snapshots`.

## Store Projector

`src/store/projector.rs` is the hot ingest path. It keeps open tool spans and
per-session file/skill/rule dedup in memory, emits small deltas, and persists
only terminal span rows. Store startup does no projector replay. The first new
event for a session lazily replays only that session before applying the delta.
Done sessions are frozen unless the legacy rebuild path is requested with
`KAIZEN_PROJECTOR=legacy`.

## Storage Topology

Each project has one canonical SQLite WAL database at
`~/.kaizen/projects/<slug>/kaizen.db`. Raw events, sessions, derived facts,
feedback, experiments, and sync state live in that database. Read commands and
the analytics query facade read SQLite directly.

Tantivy search and the GraphQLite code graph are rebuildable sidecars, not
alternate event stores. Legacy hot-event and cold-partition artifacts are
ignored. A legacy `hot/outbox.redb` is imported once into SQLite and archived.
Kaizen does not use DuckDB, Arrow, Parquet, or an rkyv hot log, and it has no
user-facing storage migration command. See
[ADR 010](adr/010-sqlite-only-local-storage.md).

The daemon can own writes for supported paths. Direct mode opens the same
SQLite database in-process, so both modes share one persistence model.

## Performance Gates

Performance budgets for representative local project data:

| Surface | Gate |
|---|---:|
| Release binary | under 35 MiB |
| Session list | under 50 ms |
| Session detail | under 100 ms |
| Session search | under 250 ms |
| Idle daemon CPU | under 0.5% |

Current project scale is far below the old 100,000-session assumption. Revisit
storage architecture only when representative measurements consistently miss
these gates or active data materially approaches that scale.

## External Boundaries

- Agent transcript dirs: Cursor, Claude Code, Codex.
- Tail agents: Gemini, Pi, Kimi, Antigravity, Goose, OpenClaw, OpenCode, Copilot CLI, Copilot VS Code, and Cursor `state.vscdb`.
- OpenClaw hook handler: `~/.openclaw/hooks/kaizen-events/handler.ts` (written by `kaizen init`,
  subscribes to `command:new`, `command:stop`, `message:received`, and related events).
- Git CLI for commit/churn/dirty facts.
- LadybugDB embedded sidecar for graph detail.
- HTTP ingest server for sync.
- Optional: HTTP proxy to model APIs ([llm-proxy.md](llm-proxy.md));
  optional: third-party telemetry when `[telemetry.exporters]` is set.

## Entry Points

- `src/main.rs` — thin binary entrypoint.
- `src/bin_kaizen/` — CLI schema, workspace resolution, and feature-grouped
  command dispatch.
- `src/store/sqlite/` — SQLite facade, schema, row mappers, sessions, events,
  sync outbox, reports, metrics, feedback, and prompt/eval persistence.
