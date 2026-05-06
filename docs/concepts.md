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

**Lifecycle events:** `EventKind::Lifecycle` carries structured behavior signals in
`payload`, with `payload.type` set to values such as `todo_write` (from Cursor `TodoWrite`
in transcripts) or `mode_transition` (when hooks or tails emit them). Hooks may set
`reject_diff: true` on `Hook` payloads for apply/reject telemetry; retro heuristic H24
reads that field.

## Collection

Two tiers:

- **Tier 1 — transcript tail.** Watches agent transcript dirs with
  `notify`, parses JSONL. Rotation and partial-line safe. Optional tail
  agents: Goose, OpenClaw, OpenCode, GitHub Copilot CLI, VS Code Copilot chat (see
  `sources.tail` in [config.md](config.md#sources), resolved from
  `~/.kaizen/config.toml`).
- **Tier 2 — hooks.** `kaizen init` patches Cursor, Claude Code, and OpenClaw hooks
  to pipe JSON events into `kaizen ingest hook`. A single global wiring in
  `~/.cursor/hooks.json` and `~/.claude/settings.json` covers every workspace — no
  per-repo files required.

**Session outcomes (opt-in):** after `Stop`, a detached child can run your test/lint command and store a row in `session_outcomes`. See [outcomes.md](outcomes.md).

**Process samples (opt-in):** if the hook sends `pid` on `SessionStart` and sampling is enabled, a detached child records CPU/memory for that PID until `Stop` or a cap. See [system-telemetry.md](system-telemetry.md).

### Channel meta tag

OpenClaw surfaces a **channel** concept (DM, Slack, sandbox, etc.) alongside sessions. When
kaizen ingests an OpenClaw session, the channel value from `sessions.json` is stored as
`meta.channel` on each event payload. Use `sessions show <id>` or the TUI detail view to
inspect it. The channel is not used for filtering today; it is metadata only.

- **LLM HTTP proxy (optional).** `kaizen proxy run` records
  `EventSource::Proxy` events for requests forwarded to Anthropic-style
  APIs. See [llm-proxy.md](llm-proxy.md).

Sources (native paths; tail agents add more):

- Cursor: `~/.cursor/projects/*/agent-transcripts/*.jsonl`
- Claude Code: `~/.claude/projects/*/*.jsonl`
- Codex: JSONL transcripts (path-configurable)

## Store

SQLite WAL at `~/.kaizen/projects/<slug>/kaizen.db`. Single-writer tokio task. Append-only
`events` + derived indexes (`tool_spans`, `file_facts`,
`repo_edges`). Graph sidecar at `~/.kaizen/projects/<slug>/codegraph.db` (SQLite + GraphQLite Cypher extension) for graph
queries. Slug = canonical workspace path with `/` replaced by `-`.

**Machine-local registry:** paths to repos you have opened or inited with Kaizen are stored in `~/.kaizen/machine.db` (or under `KAIZEN_HOME`) for `--all-workspaces` aggregation. See [config.md#machine-local-registry](config.md#machine-local-registry).

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

Heuristic engine H1–H14 over a trailing window → ranked bets by
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

## Span Hierarchy

A **tool span** covers one tool call/result pair (or hook pair). During ingest, `assign_parents()` determines containment: span P contains span C when `P.start ≤ C.start && C.end ≤ P.end`. The deepest containing ancestor becomes the parent; `depth = parent.depth + 1`.

`compute_subtree_costs()` rolls up cost and token counts bottom-up so each node carries the total cost of its entire subtree.

The hierarchy is stored in four columns on `tool_spans`: `parent_span_id`, `depth`, `subtree_cost_usd_e6`, `subtree_token_count`.

`kaizen sessions tree <id>` renders this as an ASCII forest. `get_session_span_tree` (MCP) returns the `SpanNode` JSON tree. The TUI shows a depth-indented strip below the event list.

Heuristic **H18** fires when `max_depth ≥ 4` or `max_fan_out ≥ 8`, indicating a nested tool-call loop that inflates context cost.

## Human Feedback

A `FeedbackRecord` links a score (1–5), label (`good` | `bad` | `interesting` | `bug` | `regression`), and optional free-text note to a session. Records are written to the local SQLite store and queued in the sync outbox.

Heuristic **H17** reads feedback in the retro window and fires when: ≥2 records are labelled `bad` or `regression`, or ≥5 scored sessions have a mean score ≤ 2.5. The bet surfaces the affected session ids and estimates 800 tokens saved per bad session per week.

The TUI session list shows a colored `★N` badge (red 1–2, yellow 3, green 4–5) next to sessions with feedback.

## Prompt Snapshot

At `SessionStart`, Kaizen computes a Blake3 fingerprint over the sorted contents of `CLAUDE.md`, `AGENTS.md`, `.cursor/rules/*.mdc`, and `.cursor/skills/*/SKILL.md` files. The snapshot (fingerprint + file list + sizes) is stored once per unique fingerprint. Each `SessionRecord` carries the fingerprint active when the session started.

At `SessionStop`, the prompt files are re-captured. If the fingerprint changed during the session, a `prompt_changed` event is appended with `{from_fingerprint, to_fingerprint}`.

This lets `kaizen retro` compare session outcomes (cost, error rate) across prompt versions via heuristic **H16**. See `kaizen prompt --help` and [usage.md#kaizen-prompt](usage.md#kaizen-prompt).
