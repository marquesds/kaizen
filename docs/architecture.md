# Architecture

## Module Graph

- `collect` parses agent transcripts + hooks into raw `Event`s.
- `store` owns SQLite raw tables and derived indexes (`tool_spans`,
  `file_facts`, `repo_edges`).
- `metrics` builds commit-pinned repo snapshots and Ladybug sidecar.
- `retro` consumes raw telemetry + smart metrics for heuristic bets.
- `sync` ships redacted events, tool spans, and repo snapshots.

## Data Flow

1. Transcript / hook ingest → `events`.
2. Event append rebuilds `files_touched`, `skills_used`, `tool_spans`.
3. Metrics index scans git + source tree → `repo_snapshots`,
   `file_facts`, `repo_edges`, Ladybug sidecar.
4. CLI / TUI / retro read shared smart-metric report builder.
5. Sync flushes redacted outbox rows by kind:
   `events`, `tool_spans`, `repo_snapshots`.

## External Boundaries

- Agent transcript dirs: Cursor, Claude Code, Codex.
- Git CLI for commit/churn/dirty facts.
- LadybugDB embedded sidecar for graph detail.
- HTTP ingest server for sync.

## Entry Points

- `src/main.rs` — binary entry, initializes runtime, starts server/loop
