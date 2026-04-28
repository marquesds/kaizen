# Architecture

## Module Graph

- `collect` parses agent transcripts + hooks into raw `Event`s.
- `store` owns SQLite raw tables, the incremental event projector, and derived
  indexes (`tool_spans`, `file_facts`, `repo_edges`).
- `metrics` builds commit-pinned repo snapshots and Ladybug sidecar.
- `retro` consumes raw telemetry + smart metrics for heuristic bets.
- `sync` ships redacted events, tool spans, and repo snapshots.
- `proxy` forwards LLM HTTP traffic and appends `EventSource::Proxy` rows (see
  [llm-proxy.md](llm-proxy.md)).
- `telemetry` fans out the same redacted batches to optional exporters
  (PostHog, Datadog, OTLP) when configured.
- `mcp` is the stdio MCP server; tools delegate to the same `shell` commands as
  the CLI (see [mcp.md](mcp.md)).

## Data Flow

1. Transcript, hook, or (optional) proxy ingest → `events`.
2. Event append applies projector deltas for `files_touched`, `skills_used`,
   `rules_used`, and closed/orphaned `tool_spans`.
3. Metrics index scans git + source tree → `repo_snapshots`,
   `file_facts`, `repo_edges`, Ladybug sidecar.
4. CLI / TUI / retro read shared smart-metric report builder.
5. Sync flushes redacted outbox rows by kind:
   `events`, `tool_spans`, `repo_snapshots`.

## Store Projector

`src/store/projector.rs` is the hot ingest path. It keeps open tool spans and
per-session file/skill/rule dedup in memory, emits small deltas, and persists
only terminal span rows. Store startup warms this state by replaying events for
non-`Done` sessions. Done sessions are frozen unless the legacy rebuild path is
requested with `KAIZEN_PROJECTOR=legacy`.

## External Boundaries

- Agent transcript dirs: Cursor, Claude Code, Codex.
- Tail agents: Goose (`~/.config/goose/`), OpenClaw (`~/.openclaw/agents/*/sessions/`),
  OpenCode (`~/.local/share/opencode/`), Copilot CLI, Copilot VS Code.
- OpenClaw hook handler: `~/.openclaw/hooks/kaizen-events/handler.ts` (written by `kaizen init`,
  subscribes to `command:new`, `command:stop`, `message:received`, and related events).
- Git CLI for commit/churn/dirty facts.
- LadybugDB embedded sidecar for graph detail.
- HTTP ingest server for sync.
- Optional: HTTP proxy to model APIs ([llm-proxy.md](llm-proxy.md));
  optional: third-party telemetry when `[telemetry.exporters]` is set.

## Entry Points

- `src/main.rs` — CLI, `kaizen mcp` (async server), and `kaizen tui` / sync loops
