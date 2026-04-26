# Usage

CLI reference. All commands accept `--workspace <path>` (default: cwd).

Run `kaizen --help` for grouped subcommands (Trust & observe, Operate, Improve, Integrations, Shell).

**Cache-first reads:** `sessions list`, `summary`, `insights`, `guidance`, `metrics`, and `retro` read the local workspace database first, so the common path stays fast. Pass **`--refresh`** (`-r`) when you want Kaizen to rescan external agent transcripts before rendering the command. See [config.md](config.md).

**Machine-wide aggregation:** `sessions list`, `summary`, `insights`, and `metrics` accept **`--all-workspaces`**. Kaizen records each workspace path you use (canonicalized) in a machine-local JSON list, then opens each repo’s `.kaizen/kaizen.db` and merges results in memory. Details: [config.md#machine-local-registry](config.md#machine-local-registry).

**Auto-prune:** After a **full transcript rescan** (your command used `--refresh` / MCP `refresh: true`, or the scan throttle allowed a rescan), Kaizen may delete sessions older than `[retention].hot_days` — **at most once per 24 hours**. `hot_days = 0` disables that automatic pass; use `kaizen gc` for explicit pruning. See [`[retention]` in config.md](config.md#retention).

## `kaizen doctor`

Health check: version, config paths, store open, optional Cursor/Claude hook wiring. Exit `1` if the local store cannot be opened or `.kaizen/` is not writable (useful in CI). Does not write files.

## `kaizen init`

Idempotent workspace setup. Typical effects:

| Artifact | Action |
|----------|--------|
| `.kaizen/config.toml` | Created if missing (stub with commented `[sync]`). |
| `.cursor/hooks.json` | Created or patched so `SessionStart`, `PreToolUse`, `PostToolUse`, `Stop` run `kaizen ingest hook --source cursor`. |
| `.claude/settings.json` | Created or patched with the same events for `kaizen ingest hook --source claude`. |
| `.cursor/skills/kaizen-retro/SKILL.md` | Written (or skipped if you already replaced the placeholder skill). |
| `.cursor/skills/kaizen-eval/SKILL.md` | Written (or skipped if you already replaced the placeholder skill). |
| `.kaizen/backup/*.bak` | Timestamped copy before patching an existing hooks/settings file. |

Re-running is safe. Codex, Goose, OpenCode, Copilot, and OpenClaw sessions are ingested via **transcript tail** and hooks; `init` patches **Cursor**, **Claude Code**, and **OpenClaw** hook files (writes `~/.openclaw/hooks/kaizen-events/handler.ts`).

## `kaizen outcomes`

```text
kaizen outcomes show <id> [--workspace]   # JSON row from session_outcomes (opt-in feature)
```

Requires prior measurement with `[collect.outcomes] enabled` and a completed `Stop` hook. Internal: `kaizen outcomes measure` (spawned by ingest). See [outcomes.md](outcomes.md).

## `kaizen sessions`

```bash
kaizen sessions list                     # all sessions in workspace
kaizen sessions list --json             # machine-readable
kaizen sessions list --refresh
kaizen sessions list --all-workspaces
kaizen sessions show <id>               # session metadata (id, agent, model, times, status, trace_path)
kaizen sessions tree <id>               # ASCII nested tool-span tree
kaizen sessions tree <id> --depth 3     # limit display depth
kaizen sessions tree <id> --json        # JSON SpanNode tree (subtree costs + hierarchy)
```

`sessions show` prints **one session row**, not the full event stream. For turns, tools, and live tail, use **`kaizen tui`** (or inspect the transcript path shown in `trace_path`).

`sessions tree` renders the nested tool-span tree built from `assign_parents()` during ingest. Each node shows tool name, status, and subtree cost; spans consuming >40% of session cost are flagged. The TUI shows the same tree as a depth-indented strip below the event list.

## `kaizen summary`

Roll-up of count, total USD, by-agent, by-model across all ingested
sessions.

```bash
kaizen summary --json         # same shape as the MCP `kaizen_summary` tool with json=true
kaizen summary --refresh
kaizen summary --all-workspaces
```

With `--json`, the object includes `workspace`, `stats` (counts and rollups), `cost_usd`, and when metrics data is available for the window, optional **`hotspot`** (hottest file) and **`slowest_tool`** (by p95). Multi-workspace adds a `workspaces` array.

## `kaizen gc`

Drop sessions (and dependent rows) older than `[retention].hot_days`, or override the window with `--days`. **`hot_days = 0`** disables automatic pruning; `kaizen gc` still needs an explicit positive `--days`.

```bash
kaizen gc
kaizen gc --days 14
kaizen gc --vacuum            # VACUUM after delete (slow; shrinks the DB file)
```

## `kaizen completions`

Print a shell completion script to stdout. Install (examples):

Supported shells: **bash**, **elvish**, **fish**, **powershell**, **zsh**.

```bash
kaizen completions bash  > ~/.local/share/bash-completion/completions/kaizen
kaizen completions zsh   | sudo tee /usr/local/share/zsh/site-functions/_kaizen
kaizen completions fish  > ~/.config/fish/completions/kaizen.fish
# elvish, powershell: redirect stdout to the path your shell expects for completions
```

Restart the shell or `source` your profile as appropriate for your platform.

## `kaizen insights`

Activity by day, top tools, recent sessions, and a short **Guidance** teaser (top skills/rules by observed path references in payloads).

```bash
kaizen insights
kaizen insights --refresh
kaizen insights --all-workspaces
```

## `kaizen guidance`

Per-skill and per–Cursor-rule stats over a trailing window: how many sessions referenced `.cursor/skills/...` or `.cursor/rules/*.mdc` in ingested tool payloads, share of active sessions, average cost per session vs workspace average, and on-disk inventory (including unused rules/skills). Silent Cursor injection without path mentions is not counted.

```bash
kaizen guidance
kaizen guidance --days 14
kaizen guidance --json
kaizen guidance --refresh
```

## `kaizen metrics`

Smart metrics over a trailing window.

```bash
kaizen metrics --days 7
kaizen metrics --json
kaizen metrics --all-workspaces
kaizen metrics index --force   # rebuild repo snapshot + Ladybug sidecar
```

## `kaizen tui`

Ratatui-based live session browser. List + detail view, live-tail.

## `kaizen retro`

Weekly heuristic retro. Writes `.kaizen/reports/<iso-week>.md`.

```bash
kaizen retro --days 7
kaizen retro --dry-run          # print Markdown, no file write
kaizen retro --json             # machine-readable
kaizen retro --force            # overwrite this week's report
```

Heuristics: see [retro.md](retro.md). Tuning: see
[retro-tuning.md](retro-tuning.md).

## `kaizen proxy run`

Local HTTP forwarder for Anthropic-style APIs. Records [`EventSource::Proxy` events](concepts.md)
in `.kaizen/kaizen.db` and honors `[proxy]` in config (see [config](config.md), [llm-proxy](llm-proxy.md)).

```bash
kaizen proxy run
kaizen proxy run --listen 127.0.0.1:9000
kaizen proxy run --upstream https://api.anthropic.com
```

## `kaizen ingest hook`

Reads a hook event from stdin and appends to the store. Wired by
`kaizen init`; rarely called directly.

```bash
kaizen ingest hook --source cursor < event.json
kaizen ingest hook --source claude < event.json
```

## `kaizen sync`

Flush redacted outbox to configured ingest endpoint.

```bash
kaizen sync run                 # long-running loop
kaizen sync run --once          # single flush
kaizen sync status              # outbox depth + last flush
```

Contract: [ingest-contract.md](ingest-contract.md).

## `kaizen telemetry`

Optional pluggable sinks (PostHog, Datadog, OTLP, `dev`) that receive the same redacted batches as Kaizen sync. Configure `[[telemetry.exporters]]` in `~/.kaizen/config.toml` (or workspace); see [config.md](config.md#telemetry).

```bash
kaizen telemetry configure              # append an exporter template (interactive)
kaizen telemetry print-effective-config # redacted: which fields resolve from env vs TOML
```

## `kaizen mcp`

Model Context Protocol server over stdio — **most** CLI workflows are available as MCP tools so agents (Cursor, Claude Code, Goose, OpenCode, Copilot, and so on) can query Kaizen without shelling. **CLI-only today:** `doctor`, `guidance`, `gc`, `completions`, `proxy run`, and `telemetry` subcommands — run those from a real shell. Host config and tool list: [mcp.md](mcp.md).

## `kaizen exp`

A/B experiments with bootstrap CI, SRM checks, and sequential testing.

```bash
# Size first.
kaizen exp power --metric tokens_per_session --baseline-n 50

# Create in Draft; review before starting.
kaizen exp new --name add-skill \
  --hypothesis "skill cuts tokens" \
  --change "add .cursor/skills/x" \
  --metric tokens_per_session \
  --bind git --duration-days 14 --target-pct -10
  # --bind branch --control-branch main --treatment-branch feat/x

kaizen exp start <id>           # Draft → Running
kaizen exp list
kaizen exp status <id>
kaizen exp tag <id> --session <sid> --variant treatment
kaizen exp report <id>          # markdown + bootstrap CI + sequential decision
kaizen exp report <id> --json
kaizen exp conclude <id>        # Running → Concluded
kaizen exp archive <id>         # Concluded → Archived
```

Metrics: `tokens_per_session`, `cost_per_session`, `success_rate`, `tool_loops`,
`duration_minutes`, `files_per_session`, `success_rate_by_prompt`, `cost_by_prompt`.
Details: [experiments.md](experiments.md).

## `kaizen eval`

LLM-as-a-Judge evaluations. Requires `[eval].enabled = true` in config and either
`[eval].api_key` or `ANTHROPIC_API_KEY` in the environment. See [config.md#eval](config.md#eval).

```bash
kaizen eval run                       # evaluate unevaluated sessions (last 7 days, cost >= $0.01)
kaizen eval run --since-days 14       # extend the lookback window
kaizen eval run --dry-run             # print which sessions would be evaluated without calling the judge
kaizen eval list                      # list all stored eval results
kaizen eval list --min-score 0        # include all sessions (default: 0.0 = show all)
kaizen eval list --json               # emit JSON array of EvalRow objects
kaizen eval prompt <session-id>       # print the rendered judge prompt (no LLM call)
kaizen eval prompt <session-id> --rubric tool-efficiency-v1  # explicit rubric
```

**`kaizen eval run`** finds sessions that have no existing eval, calls the configured judge model
(default: `claude-haiku-4-5-20251001`) with the `tool-efficiency-v1` rubric, stores the result,
and prints a summary. Sessions with `score < 0.4` are flagged and surfaced by **H15** in `kaizen retro`.

**`kaizen eval prompt`** renders the full judge prompt for any session without making any LLM
call — useful for manual review, piping to an external model, or debugging rubric output.

**`kaizen sessions show <id>`** appends eval rows when present, and prints the active `prompt_fingerprint` plus the list of tracked files when a snapshot is stored.

## `kaizen prompt`

Prompt/system-prompt version tracking. Each `SessionStart` hook captures a Blake3 fingerprint of your `CLAUDE.md`, `AGENTS.md`, `.cursor/rules/*.mdc`, and `.cursor/skills/*/SKILL.md` files. If the prompt changes between session start and session end, a `prompt_changed` event is recorded.

```bash
kaizen prompt list                       # list all stored prompt snapshots
kaizen prompt list --json                # JSON array
kaizen prompt show <fingerprint>         # show files in a snapshot
kaizen prompt show <fingerprint> --json
kaizen prompt diff <fp_a> <fp_b>         # lines added (+), removed (-), changed (~)
```

**H16** in `kaizen retro` surfaces when ≥2 prompt versions each have ≥5 sessions and one underperforms the other by >20% on cost or >15% on error rate.

## `kaizen sessions annotate` and `kaizen feedback`

Attach human feedback (score 1–5, label, free-text note) to any session, then query the collected feedback.

```bash
# Annotate a session
kaizen sessions annotate <id> --score 2 --label bad --note "hallucinated file path"
kaizen sessions annotate <id> --label interesting

# List feedback
kaizen feedback list                     # all records
kaizen feedback list --label bad         # filter by label
kaizen feedback list --since 7d          # last 7 days
kaizen feedback list --since 7d --json   # JSON array
```

**Labels:** `good`, `bad`, `interesting`, `bug`, `regression`.

**Score:** integer 1–5 (1 = worst, 5 = best).

**`kaizen sessions show <id>`** appends any feedback record for that session when present.

**H17** in `kaizen retro` fires when ≥2 bad/regression records are present in the window, or when ≥5 scored sessions have a mean score ≤ 2.5.
