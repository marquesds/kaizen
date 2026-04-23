# Concepts

**New to the mental model?** Read [telemetry-journey.md](telemetry-journey.md) for the
end-to-end path from agent activity to stored facts, then return here for definitions.

---

Shared vocabulary. Deep dives live in dedicated docs.

## Session

One agent run end-to-end. Has `id`, `agent` (cursor|claude|codex),
`model`, `workspace`, `started_at`, `ended_at`, `status`
(`running|waiting|idle|done`). Sessions own events, tool spans, file
facts, cost.

## Event

Atomic telemetry record: prompt, assistant turn, tool call, hook fire,
session lifecycle. Ordered by `event_seq` within a session. See
[datamodel.md](datamodel.md).

## Collection

Two tiers:

- **Tier 1 — transcript tail.** Watches agent transcript dirs with
  `notify`, parses JSONL. Rotation and partial-line safe. Optional tail
  agents: Goose, OpenCode, GitHub Copilot CLI, VS Code Copilot chat (see
  `sources.tail` in [config.md](config.md#sources), resolved from
  `~/.kaizen/config.toml`).
- **Tier 2 — hooks.** `kaizen init` patches Cursor + Claude Code hooks
  to pipe JSON events into `kaizen ingest hook`.

- **LLM HTTP proxy (optional).** `kaizen proxy run` records
  `EventSource::Proxy` events for requests forwarded to Anthropic-style
  APIs. See [llm-proxy.md](llm-proxy.md).

Sources (native paths; tail agents add more):

- Cursor: `~/.cursor/projects/*/agent-transcripts/*.jsonl`
- Claude Code: `~/.claude/projects/*/*.jsonl`
- Codex: JSONL transcripts (path-configurable)

## Store

SQLite WAL at `.kaizen/kaizen.db`. Single-writer tokio task. Append-only
`events` + derived indexes (`tool_spans`, `file_facts`,
`repo_edges`). Graph sidecar at `.kaizen/codegraph.db` (SQLite + GraphQLite Cypher extension) for graph
queries.

## Redact

Every outbound event passes `redact`: secrets (Aho-corasick), env vars,
absolute paths, git emails. Verified by
`specs/redaction-completeness.qnt`.

## Sync

Reads `sync_outbox`, batches (500 events / 1 MB / 10 s), UUIDv7
idempotency key, retry + backoff, HTTPS POST. Dedup on
`(team_id, workspace_hash, session_id_hash, event_seq)`. Contract:
[ingest-contract.md](ingest-contract.md).

## Retro

Heuristic engine H1–H8 over a trailing window → ranked bets by
`tokens_saved_per_week / (effort_minutes + 1)`. Output: Markdown +
JSON. No LLM calls. See [retro.md](retro.md), tuning in
[retro-tuning.md](retro-tuning.md).

## Experiment

Hypothesis test: `Draft → Running → Concluded → Archived`. Binding
either `git` (walk control/treatment commits) or `manual` (tag per
session). Bootstrap CI (10k resamples, 95% interval) on median delta.
See [experiments.md](experiments.md).

## Cost

Price table in bundled `cost.toml`. Claude / Codex: native token
counts. Cursor: model+turns heuristic (no native tokens). Adjust the
table to match your contract prices.
