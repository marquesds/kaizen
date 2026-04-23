# Datamodel

## Core Entities

- `SessionRecord`
  session metadata plus repo binding: `start_commit`, `end_commit`,
  `branch`, `dirty_start`, `dirty_end`, `repo_binding_source`.
- `Event`
  raw session event with exact-or-null `tokens_in`, `tokens_out`,
  `reasoning_tokens`, `tool_call_id`, `ts_exact`.
- `tool_spans`
  derived per-tool spans. One row per correlated tool execution.
- `repo_snapshots`
  commit-pinned code fact snapshot for one workspace fingerprint.
- `file_facts`
  compact file-level smart metrics used by CLI, TUI, retro, sync.
- `repo_edges`
  graph edges for dependency, co-change, and call relations.

## Invariants

- Raw `events` append-only. Derived tables can rebuild from them.
- Token and reasoning fields stay exact-or-null. No synthetic backfill.
- `tool_spans.status` in `done|orphaned`.
- One `file_facts` row per `(snapshot_id, path)`.
- `repo_snapshots.id` changes when commit, dirty fingerprint, or analyzer
  version changes.

## Relationships

- `sessions 1:N events`
- `sessions 1:N tool_spans`
- `sessions 1:1 session_repo_binding`
- `repo_snapshots 1:N file_facts`
- `repo_snapshots 1:N repo_edges`
