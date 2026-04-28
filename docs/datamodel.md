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
  derived per-tool spans. One row per closed or orphaned correlated tool
  execution. Open spans live in the incremental projector until close/flush.
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
- Incremental projector owns hot derived writes for `tool_spans`, `files_touched`,
  `skills_used`, and `rules_used`; `KAIZEN_PROJECTOR=legacy` restores full
  per-session span rebuild.
- Session browsing reads through `list_sessions_page`: SQL filters by
  workspace, optional lower-case agent prefix, status, and `started_at_ms`
  floor before applying `LIMIT/OFFSET`.
- Event browsing reads through `list_events_page`: `after_seq` is inclusive,
  so callers start at `0` and continue from `last_seq + 1`.
- Token and reasoning fields stay exact-or-null. No synthetic backfill.
- `tool_spans.status` in `done|orphaned`.
- Running-session open spans may be absent from `tool_spans` until a post hook,
  tool result, `Stop`, or one-hour orphan TTL flush.
- One `file_facts` row per `(snapshot_id, path)`.
- `repo_snapshots.id` changes when commit, dirty fingerprint, or analyzer
  version changes.

## Query Indexes

- `sessions(workspace, started_at_ms DESC, id ASC)` keeps session pages stable
  and newest-first.
- `sessions(workspace, lower(agent), started_at_ms DESC, id ASC)` supports
  case-insensitive agent-prefix filtering.
- `events(session_id, seq)` supports deterministic event pages inside one session.

## Relationships

- `sessions 1:N events`
- `sessions 1:N tool_spans`
- `sessions 1:1 session_repo_binding`
- `repo_snapshots 1:N file_facts`
- `repo_snapshots 1:N repo_edges`
