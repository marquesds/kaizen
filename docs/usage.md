# Usage

CLI reference. All commands accept `--workspace <path>` or `--project <name>` to select a workspace (default: cwd). The two flags are mutually exclusive.

Run `kaizen --help` for grouped subcommands (Trust & observe, Operate, Improve, Integrations, Shell).

**Daemon mode:** `kaizen init` starts a local daemon for capture and supported
read/write paths. Use `kaizen daemon status`, `kaizen daemon stop`, or
`--no-daemon` / `KAIZEN_DAEMON=0` for direct SQLite mode. See
[daemon.md](daemon.md). Daemon mode also serves the loopback web console printed
as `web: http://127.0.0.1:<port>/?token=<token>` by `kaizen daemon start --background`
and `kaizen daemon status`.

**Cache-first reads:** `sessions list`, `summary`, `insights`, `guidance`, `metrics`, `retro`, **`exp report`**, and **`exp power`** read the local workspace database first and avoid transcript scans. Pass **`--refresh`** (`-r`) when that read should rescan external agent transcripts before rendering. Use **`kaizen load`** when you want an explicit backfill of previous sessions without coupling it to a report. Both can take a while on large workspaces; with `--source provider|mixed`, `--refresh` can also refresh remote provider cache. See [config.md](config.md).

## Selecting a project

Every command resolves a workspace through one of three mechanisms, applied in order:

| Flag | Behavior |
|------|----------|
| _(none)_ | Uses the current working directory. |
| `--project <NAME>` | Resolves a registered workspace by short name or slug (e.g. `kaizen`, `my-app`). Run `kaizen projects list` to see names and their paths. |
| `--workspace <PATH>` | Explicit absolute path; always takes precedence when given. |

`--project` and `--workspace` are mutually exclusive; passing both is an error.

**Machine-wide aggregation:** `sessions list`, `summary`, `insights`, and `metrics` accept **`--all-workspaces`**. `kaizen load` defaults to all registered workspaces. Kaizen records each workspace path you use (canonicalized) in a machine-local registry, then opens each repo’s local store and merges or loads results. Details: [config.md#machine-local-registry](config.md#machine-local-registry).

**Auto-prune:** After a **full transcript rescan** (your command used `--refresh` / MCP `refresh: true`, or the scan throttle allowed a rescan), Kaizen may delete sessions older than `[retention].hot_days` — **at most once per 24 hours**. `hot_days = 0` disables that automatic pass; use `kaizen gc` for explicit pruning. See [`[retention]` in config.md](config.md#retention).

## `kaizen doctor`

Health check: version, config paths, store open, optional Cursor/Claude hook wiring. Exit `1` if the project data dir is not writable (useful in CI). Does not write files.

## `kaizen init`

Idempotent workspace setup. Typical effects:

| Artifact | Action |
|----------|--------|
| `~/.kaizen/projects/<slug>/config.toml` | Created if missing (stub with commented `[sync]`). |
| `~/.cursor/hooks.json` | Created or patched so `SessionStart`, `PreToolUse`, `PostToolUse`, `Stop` run `kaizen ingest hook --source cursor`. |
| `~/.claude/settings.json` | Created or patched with the same events for `kaizen ingest hook --source claude`. |
| `~/.cursor/skills/kaizen-retro/SKILL.md` | Written (or skipped if you already replaced the placeholder skill). |
| `~/.cursor/skills/kaizen-eval/SKILL.md` | Written (or skipped if you already replaced the placeholder skill). |
| `~/.kaizen/projects/<slug>/backup/*.bak` | Timestamped copy before patching an existing hooks/settings file. |

Re-running is safe. Codex, Goose, OpenCode, Copilot, and OpenClaw sessions are ingested via **transcript tail** and hooks; `init` patches **Cursor**, **Claude Code**, and **OpenClaw** hook files (writes `~/.openclaw/hooks/kaizen-events/handler.ts`).

`init` also asks the daemon to start workspace capture. The daemon keeps a
periodic transcript scanner alive for the workspace, so normal use does not
require `kaizen observe`.

```bash
kaizen init          # hooks + daemon scanner capture
kaizen init --deep   # also start daemon proxy tasks and report deep-capture readiness
```

`--deep` is opt-in. It starts loopback proxy endpoints where possible, but does
not silently rewrite agent model-provider config when Kaizen cannot verify a
supported setting. In that case init reports partial deep capture and hooks/tail
capture remain active.

## `kaizen projects`

Manage the registry of workspaces known to Kaizen.

```bash
kaizen projects list          # all registered workspaces: name, slug, path
kaizen projects list --json   # machine-readable
```

`kaizen projects list` shows each workspace's short name (usable with `--project`), its slug, and the canonical path. A workspace is registered automatically the first time `kaizen init` or any workspace-scoped command runs against it.

## `kaizen load`

Explicitly loads previous local agent sessions from machine transcript stores into Kaizen. It scans registered workspace roots by default and keeps sessions repo-scoped by transcript `cwd` metadata where the agent provides it.

```bash
kaizen load                         # load all registered workspaces
kaizen load --workspace /repo --json
kaizen load --project kaizen
kaizen sessions load --json         # alias under sessions
```

Use `load` after installing or upgrading Kaizen when existing Codex, Claude Code, Cursor, OpenClaw, Goose, OpenCode, or Copilot sessions should appear in reports. Use `sessions list --refresh` when you want one read command to rescan before rendering.

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
kaizen sessions load --json            # alias for `kaizen load --json`
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

Structured expressions with known fields (`tool:bash`, `tokens_total:>5000`,
`feedback_label:bad`, etc.) are routed through the local trace query engine
instead of BM25.

## `kaizen query`

Structured trace query over local events. Version 1 supports `AND` terms and
fields: `agent`, `model`, `kind`, `tool`, `path`, `skill`, `tokens_total`,
`cost_usd`, `eval_score`, `feedback_label`, `prompt`, `status`, `span_kind`.

```bash
kaizen query 'tool:bash AND tokens_total:>5000'
kaizen query 'feedback_label:bad' --since 30d --json
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

Upgrade kaizen to the latest release. Detects the install method from the running binary path. Non-Homebrew installs download the GitHub release binary and verify its `.sha256` asset so upgrades do not compile DuckDB locally.

| Install method | Command run |
|---|---|
| Homebrew (`/Cellar/kaizen-cli`, `/opt/homebrew/`, `/usr/local/Cellar/`) | `brew upgrade kaizen-cli` |
| Release / Cargo-path binary (default) | Download, verify, and replace with the latest GitHub release binary |
| Source fallback | `kaizen upgrade --from-source` runs `cargo install kaizen-cli --locked --force` |

```bash
kaizen upgrade
```

Homebrew and source fallback subprocess output streams directly to the terminal. Exits non-zero if download, checksum verification, binary replacement, or the fallback subprocess fails.

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
kaizen guidance score --days 30 --min-sessions 30
kaizen guidance propose --artifact skill:tdd
kaizen guidance propose --artifact rule:style --apply
kaizen guidance candidates list
kaizen guidance candidates validate <candidate_id>
```

`guidance score` adds deterministic evaluation from stored outcomes: eval rows,
human feedback, test/lint outcomes, cost, tokens, and repeated tool-call loops.
Rows include a deterministic 70/30 train/held-out validation split, validation
gate, and generalization gap. Rows are marked `stale`, `insufficient_evidence`,
or `current`. `guidance propose` creates a candidate for one artifact; `--apply`
backs the file up under
`.kaizen/backup/guidance/<candidate_id>/`, mutates only that artifact, captures a
new prompt snapshot, and creates a prompt-bound experiment when possible.
Rejected candidates are remembered per artifact, so later proposals can avoid
repeating harmful edits.
`guidance candidates validate` reads that experiment: proven improvement marks
the candidate `validated`, proven miss or guardrail regression marks it
`rejected`, and missing arm/evidence leaves it `applied`.

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

Local HTTP forwarder for Anthropic-style and OpenAI-compatible APIs. Records [`EventSource::Proxy` events](concepts.md)
in `~/.kaizen/projects/<slug>/kaizen.db` and honors `[proxy]` in config (see [config](config.md), [llm-proxy](llm-proxy.md)).

```bash
kaizen proxy run
kaizen proxy run --listen 127.0.0.1:9000
kaizen proxy run --upstream https://api.anthropic.com
kaizen proxy run --provider openai
kaizen observe --agent codex -- codex
```

For normal collection, prefer `kaizen init`. `kaizen observe` is a daemon-backed
debug/manual wrapper that injects session and proxy env into one child command.

Use `kaizen sessions trace <id>` for proxy-backed LLM spans, and
`kaizen metrics quality --json` to inspect field coverage and trace-correlation
health.

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

Pluggable sinks receive the same redacted batches as Kaizen sync. The default Cargo build ships PostHog, Datadog, and OTLP exporters; `dev` tracing stays opt-in (`--features telemetry-dev`). The **`file`** sink appends one summary JSON line per batch to **`~/.kaizen/projects/<slug>/telemetry.ndjson`** (optional `path` override). `[[telemetry.exporters]]` in `~/.kaizen/config.toml` is the configuration surface; see [config.md](config.md#telemetry).

`kaizen telemetry configure` is a **validating wizard**: it resolves credentials from the matching env vars or `--api-key` / `--site` / `--host` / `--endpoint` flags, runs a live `health` check against the provider, and only writes the TOML row when the check succeeds. For Datadog the wizard also rejects `ddapp_*` Application Keys before the network call (only the 32-hex-char API Key works for `DD-API-KEY`). When the resolved provider is `datadog` or `posthog`, the wizard idempotently sets `[telemetry.query].provider` so `pull` works without further config; an existing `[telemetry.query]` table is left alone. Re-running for the same exporter type is a no-op (no duplicate row). With `--non-interactive` it never prompts and fails on any missing value.

`kaizen telemetry test` sends one synthetic redacted event to every configured sink and reports per-exporter ok/fail. Telemetry-only flows (`push`, `test`) auto-generate `~/.kaizen/local_salt.hex` (`chmod 0o600`) when `[sync].team_salt_hex` is empty, so you do not need a Kaizen account to fan out to third-party providers.

`kaizen telemetry pull` is implemented for Datadog (Logs Search v2). It reads `DD_API_KEY` from the matching `[[telemetry.exporters]]` row first (no need to also export it as env if `configure` already wrote it) and falls back to `DD_API_KEY` / `DD_SITE` env. `DD_APP_KEY` is env-only (DD Application Keys are user-scoped; we deliberately do not persist them in TOML). PostHog pull is still a stub; OTLP is export-only by design.

```bash
# Validating wizard. Reads DD_API_KEY/DD_SITE (or prompts), runs /api/v1/validate, then writes.
kaizen telemetry configure --type datadog --site us5.datadoghq.com --non-interactive

# Same for PostHog; reads POSTHOG_API_KEY/POSTHOG_HOST (or prompts) and pings the host.
kaizen telemetry configure --type posthog --host https://us.i.posthog.com --non-interactive

# File or OTLP sinks (no provider credential prompt).
kaizen telemetry configure --type file --path telemetry.ndjson
kaizen telemetry configure --type otlp --endpoint http://127.0.0.1:4318

# Send one synthetic event to every configured sink (per-sink ok/fail report).
kaizen telemetry test

# Inspect resolution and provider health.
kaizen telemetry print-effective-config   # redacted: which fields resolve from env vs TOML
kaizen telemetry doctor                   # call provider health, show query authority
kaizen telemetry pull --days 1            # one Logs Search page; needs DD_APP_KEY for DD

# Replay SQLite events through every configured exporter (no Kaizen POST).
kaizen telemetry push

# NDJSON file sink helpers.
kaizen telemetry tail                       # read NDJSON from the file exporter (default path)
kaizen telemetry tail --no-follow           # print once and exit; missing default file is empty
kaizen telemetry tail --file /tmp/t.ndjson  # absolute or relative to workspace
kaizen telemetry tail --json                # pretty-print each JSON line
```

## `kaizen mcp`

Model Context Protocol server over stdio — **most** CLI workflows are available as MCP tools so agents (Cursor, Claude Code, Goose, OpenCode, Copilot, and so on) can query Kaizen without shelling. **CLI-only today:** `doctor`, `guidance`, `gc`, `completions`, `proxy run`, and all `kaizen telemetry` subcommands (including the new `configure` wizard, `test`, and Datadog `pull`) — run those from a real shell. Host config and tool list: [mcp.md](mcp.md).

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
  # --bind prompt --control-fingerprint <fp> --treatment-fingerprint <fp>

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

## `kaizen cases`, `kaizen rules`, `kaizen alerts`, `kaizen review`

Local trace-to-case loop inspired by LangWatch/LangSmith patterns. Automation
is local-only: rules create cases, queue review items, or emit local alerts.

```bash
kaizen cases mine --since 14d
kaizen cases create --session <id> --reason "bad tool loop" --label regression
kaizen cases list --json
kaizen cases show <case-id>
kaizen cases archive <case-id>

kaizen rules create --name shell-loops --filter 'tool:bash' \
  --action queue_review --message "review shell-heavy session"
kaizen rules run --since 7d
kaizen rules disable <rule-id>
kaizen rules enable <rule-id>

kaizen alerts check --days 7 --json

kaizen review list
kaizen review show <review-id>
kaizen review resolve <review-id>
kaizen review dismiss <review-id>
```

Built-in alerts cover cost spikes, eval regression, bad feedback, error-rate
spikes, context pressure, retry cascades, and max-token truncation rate.

## `kaizen eval`

LLM-as-a-Judge evaluations. Requires `[eval].enabled = true` in config and either
`[eval].api_key` or `ANTHROPIC_API_KEY` in the environment. See [config.md#eval](config.md#eval).

```bash
kaizen eval run                       # evaluate unevaluated sessions (last 7 days, cost >= $0.01)
kaizen eval run --since-days 14       # extend the lookback window
kaizen eval run --dry-run             # print which sessions would be evaluated without calling the judge
kaizen eval run --dry-run --json      # machine-readable candidate sessions
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
