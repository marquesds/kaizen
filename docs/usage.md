# Usage

CLI reference. All commands accept `--workspace <path>` (default: cwd).

Run `kaizen --help` for grouped subcommands (Trust & observe, Operate, Improve, Integrations, Shell).

**Daemon mode:** Kaizen starts a local daemon for supported read/write paths.
Use `kaizen daemon status`, `kaizen daemon stop`, or `--no-daemon` /
`KAIZEN_DAEMON=0` for direct SQLite mode. See [daemon.md](daemon.md).

**Cache-first reads:** `sessions list`, `summary`, `insights`, `guidance`, `metrics`, `retro`, **`exp report`**, and **`exp power`** read the local workspace database first and avoid transcript scans. Pass **`--refresh`** (`-r`) only when you want Kaizen to rescan external agent transcripts before rendering. That can take a while on large workspaces; with `--source provider|mixed`, it can also refresh remote provider cache. See [config.md](config.md).

**Machine-wide aggregation:** `sessions list`, `summary`, `insights`, and `metrics` accept **`--all-workspaces`**. Kaizen records each workspace path you use (canonicalized) in a machine-local JSON list, then opens each repo’s `.kaizen/kaizen.db` and merges results in memory. Details: [config.md#machine-local-registry](config.md#machine-local-registry).

**Auto-prune:** After a **full transcript rescan** (your command used `--refresh` / MCP `refresh: true`, or the scan throttle allowed a rescan), Kaizen may delete sessions older than `[retention].hot_days` — **at most once per 24 hours**. `hot_days = 0` disables that automatic pass; use `kaizen gc` for explicit pruning. See [`[retention]` in config.md](config.md#retention).

## `kaizen doctor`

Health check: version, config paths, store open, optional Cursor/Claude hook wiring. Exit `1` if the project data dir is not writable (useful in CI). Does not write files.

## `kaizen init`

Idempotent workspace setup. Typical effects:

| Artifact | Action |
|----------|--------|
| `~/.kaizen/projects/<slug>/config.toml` | Created if missing (stub with commented `[sync]`). |
| `.cursor/hooks.json` | Created or patched so `SessionStart`, `PreToolUse`, `PostToolUse`, `Stop` run `kaizen ingest hook --source cursor`. |
| `.claude/settings.json` | Created or patched with the same events for `kaizen ingest hook --source claude`. |
| `.cursor/skills/kaizen-retro/SKILL.md` | Written (or skipped if you already replaced the placeholder skill). |
| `.cursor/skills/kaizen-eval/SKILL.md` | Written (or skipped if you already replaced the placeholder skill). |
| `~/.kaizen/projects/<slug>/backup/*.bak` | Timestamped copy before patching an existing hooks/settings file. |

Re-running is safe. Codex, Goose, OpenCode, Copilot, and OpenClaw sessions are ingested via **transcript tail** and hooks; `init` patches **Cursor**, **Claude Code**, and **OpenClaw** hook files (writes `~/.openclaw/hooks/kaizen-events/handler.ts`).

## `kaizen outcomes`

```text
kaizen outcomes show <id> [--workspace]   # JSON row from session_outcomes (opt-in feature)
```

Requires prior measurement with `[collect.outcomes] enabled` and a completed `Stop` hook. Internal: `kaizen outcomes measure` (spawned by ingest). See [outcomes.md](outcomes.md).

## `kaizen sessions`

```bash
kaizen sessions list                     # latest 100 sessions in workspace
kaizen sessions list --json             # machine-readable
kaizen sessions list --limit 20        # cap rows after sort (newest first)
kaizen sessions list --limit 0         # full output; not part of the fast-read budget
kaizen sessions list --refresh
kaizen sessions list --all-workspaces
kaizen sessions show <id>               # session metadata (id, agent, model, times, status, trace_path)
kaizen sessions tree <id>               # ASCII nested tool-span tree
kaizen sessions tree <id> --depth 3     # limit display depth
kaizen sessions tree <id> --json        # JSON SpanNode tree (subtree costs + hierarchy)
kaizen sessions search "deadlock" --since 7d --agent claude-code --limit 50
kaizen sessions search 'path:src/store/sqlite.rs' --kind tool_use
kaizen sessions search 'skill:caveman AND tokens_total:>5000'
```

`sessions show` prints **one session row**, not the full event stream. For turns, tools, and live tail, use **`kaizen tui`** (or inspect the transcript path shown in `trace_path`).

`sessions tree` renders the nested tool-span tree built from `assign_parents()` during ingest. Each node shows tool name, status, and subtree cost; spans consuming >40% of session cost are flagged. When a session exists but has no tool spans yet, text output prints a `(no tool spans for session <id>)` placeholder while `--json` returns `[]`. The TUI shows the same tree as a depth-indented strip below the event list.

`sessions search` uses the project Tantivy index at `~/.kaizen/projects/<slug>/search/`. It indexes redacted event text for messages, tool calls, and tool results. Payload bodies are not stored in the index; result snippets are rebuilt from persisted events. If the index is missing or corrupt, Kaizen returns a fast error instead of scanning the full event table. Rebuild with:

```bash
kaizen search reindex
```

## `kaizen summary`

Roll-up of count, total USD, by-agent, by-model across all ingested
sessions.

**Cost:** `summary` sums `cost_usd_e6` on stored events. Transcript ingest **estimates** USD from line-level token usage (bundled price table) when usage fields are present; Kaizen **proxy** `Cost` events remain authoritative when you use the proxy. After upgrading ingest logic, run **`kaizen summary --refresh`** (or `sessions list --refresh`) so existing workspaces rescan transcripts and backfill estimates.

```bash
kaizen summary --json         # same shape as the MCP `kaizen_summary` tool with json=true
kaizen summary --refresh
kaizen summary --all-workspaces
```

With `--json`, the object includes `workspace`, `stats` (counts and rollups), `cost_usd`, optional **`cost_note`** when sessions exist but stored cost rollup is zero (same situation as [cost rollup zero](#cost-shows-zero): no `cost_usd_e6` on events), and when metrics data is available for the window, optional **`hotspot`** (hottest file) and **`slowest_tool`** (by p95). Multi-workspace adds a `workspaces` array.

<a id="cost-shows-zero"></a>

### When cost rollup is zero

If **`kaizen summary`** shows **$0.00** but session count is above zero, events in the store have no **`cost_usd_e6`**. That is normal when **Cursor** `agent-transcripts` JSONL lines omit **`tokens`** / **`usage`**. You can still get accurate spend from **Claude** or **Codex** transcripts that include usage, from **hooks** that send **`total_cost_usd`**, or from the **Kaizen LLM proxy** (authoritative **`Cost`** events). After wiring or changing ingest, run **`kaizen summary --refresh`** so the store rescans.

## `kaizen upgrade`

Upgrade kaizen to the latest release. Detects the install method from the running binary path and delegates to the right tool — no flags required.

| Install method | Command run |
|---|---|
| Homebrew (`/Cellar/kaizen-cli`, `/opt/homebrew/`, `/usr/local/Cellar/`) | `brew upgrade kaizen-cli` |
| Cargo (default) | `cargo install kaizen-cli --locked --force` |

```bash
kaizen upgrade
```

Subprocess output streams directly to the terminal. Exits non-zero if the underlying tool fails.

## `kaizen gc`

Drop sessions (and dependent rows) older than `[retention].hot_days`, or override the window with `--days`. **`hot_days = 0`** disables automatic pruning; `kaizen gc` still needs an explicit positive `--days`.

```bash
kaizen gc
kaizen gc --days 14
kaizen gc --vacuum            # VACUUM after delete (slow; shrinks the DB file)
```

## `kaizen migrate`

Bootstrap or roll back tiered storage for a workspace. `v2` exports existing SQLite events into `hot/log.bin` plus daily Parquet files under `cold/events/` inside the project data dir (`~/.kaizen/projects/<slug>/`), and keeps a `kaizen.db.v1.bak` backup. `v1` restores raw SQLite events from the hot log and cold partitions.

```bash
kaizen migrate v2
kaizen migrate v2 --allow-skew
kaizen migrate v1
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

Weekly heuristic retro. Writes `~/.kaizen/projects/<slug>/reports/<iso-week>.md`.

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
in `~/.kaizen/projects/<slug>/kaizen.db` and honors `[proxy]` in config (see [config](config.md), [llm-proxy](llm-proxy.md)).

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

Pluggable sinks receive the same redacted batches as Kaizen sync. Use **`type = "file"`** to append one summary JSON line per batch to **`~/.kaizen/projects/<slug>/telemetry.ndjson`** (optional `path` override); PostHog, Datadog, OTLP, and `dev` are optional and may need Cargo build features. Configure `[[telemetry.exporters]]` in `~/.kaizen/config.toml`; see [config.md](config.md#telemetry).

```bash
kaizen telemetry configure                # append an exporter template (interactive)
kaizen telemetry configure --type file --path telemetry.ndjson  # noninteractive file sink
kaizen telemetry print-effective-config  # redacted: which fields resolve from env vs TOML
kaizen telemetry push                     # replay SQLite events through exporters (no Kaizen POST)
kaizen telemetry tail                    # read NDJSON from the file exporter (default path above)
kaizen telemetry tail --no-follow        # print the file once and exit; missing default file is empty
kaizen telemetry tail --file /tmp/t.ndjson  # path (absolute or relative to workspace)
kaizen telemetry tail --json            # pretty-print each JSON line
```

## `kaizen mcp`

Model Context Protocol server over stdio — **most** CLI workflows are available as MCP tools so agents (Cursor, Claude Code, Goose, OpenCode, Copilot, and so on) can query Kaizen without shelling. **CLI-only today:** `doctor`, `guidance`, `gc`, `completions`, `proxy run`, and all `kaizen telemetry` subcommands — run those from a real shell. Host config and tool list: [mcp.md](mcp.md).

## `kaizen exp`

A/B experiments with bootstrap CI, SRM checks, and sequential testing.

```bash
# Size first. Reads are cache-first; use --refresh only when the store may be stale.
kaizen exp power --metric tokens_per_session --baseline-n 50
kaizen exp power --metric tokens_per_session --baseline-n 50 --refresh

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
kaizen exp report <id> --refresh   # full transcript rescan before report (slow; use when store may be stale)
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
