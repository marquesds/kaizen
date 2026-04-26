# Datamodel

## Core Entities

- `SessionRecord`
  session metadata plus repo binding: `start_commit`, `end_commit`,
  `branch`, `dirty_start`, `dirty_end`, `repo_binding_source`,
  `prompt_fingerprint`, optional `parent_session_id` (subagent → parent),
  optional env fields from hooks (`agent_version`, `os`, `arch`), optional
  `repo_file_count` / `repo_total_loc` (for future aggregation from `file_facts`).
- `Event`
  raw session event with exact-or-null `tokens_in`, `tokens_out`,
  `reasoning_tokens`, `tool_call_id`, `ts_exact`, proxy quality fields
  (`stop_reason`, `latency_ms`, `ttft_ms`, `retry_count`, context and cache
  token splits), and `EventKind::Lifecycle` for `payload.type`-discriminated
  behavior signals.
- `tool_spans`
  derived per-tool spans. One row per correlated tool execution.
- `repo_snapshots`
  commit-pinned code fact snapshot for one workspace fingerprint.
- `file_facts`
  compact file-level smart metrics used by CLI, TUI, retro, sync.
- `repo_edges`
  graph edges for dependency, co-change, and call relations.
- `session_outcomes` (opt-in)
  one row per session after a post-`Stop` worker runs configured test/lint commands;
  nullable test counts, lint/build/PR/CI fields for future population; `measured_at_ms` required.
- `session_samples` (opt-in)
  per-PID time series while the hook-provided process is live: `ts_ms`, `cpu_percent`, `rss_bytes`.
  Stops when a workspace stop file appears, cap reached, or process exits.

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
