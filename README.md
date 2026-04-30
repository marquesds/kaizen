# kaizen

Kaizen watches your coding agents work, locally, across Cursor, Claude Code,
Codex, OpenClaw, Goose, OpenCode, and Copilot.

At the end of the week, it tells you what's wasting tokens, causing loops,
or making agents worse in this repo.

Then it lets you test the fix: "add this rule", "split this file", or
"delete this unused skill" becomes measurable instead of vibes.

[![crates.io](https://img.shields.io/crates/v/kaizen-cli.svg)](https://crates.io/crates/kaizen-cli)
[![CI](https://github.com/marquesds/kaizen/actions/workflows/ci.yml/badge.svg)](https://github.com/marquesds/kaizen/actions/workflows/ci.yml)
[![License: AGPL v3](https://img.shields.io/badge/License-AGPL_v3-blue.svg)](LICENSE)
[![Sponsor](https://img.shields.io/badge/Sponsor-FF5E5B?logo=ko-fi&logoColor=white)](https://ko-fi.com/lucasmarques)

## Demo

```markdown
# Kaizen Retro — Week 2026-W17

Span: 2026-04-20 → 2026-04-27 · Sessions: 9 · Cost: $12.40

## High-Confidence Bet

### 1. Stabilize failing shell commands (H10 · High · investigation)
- Hypothesis: Three sessions hit repeated failing test or shell results before recovery.
- Evidence: 3 failing command clusters · sessions s_42, s_45, s_51
- Saves ~1200 tokens/week (est.) · Confidence: High
- Effort: 45 min · Apply: Add a repo-local smoke command and document expected env vars.

## To Investigate

### 2. Split `src/store/projector.rs` from `src/shell/retro.rs` (H2 · Medium · investigation)
- Hypothesis: These files are edited together in four sessions, which suggests hidden coupling.
- Evidence: Co-edit count: 4 · combined complexity: 88
- Saves ~7280 tokens/week (est.) · Confidence: Medium
- Effort: 120 min · Apply: Extract shared projection logic behind a smaller interface.

### 3. Reduce large file reads in `src/main.rs` (H12 · High · investigation)
- Hypothesis: Agents repeatedly read a 900+ LOC command file before small CLI edits.
- Evidence: 5 read-like calls · 934 LOC
- Saves ~9340 tokens/week (est.) · Confidence: High
- Effort: 40 min · Apply: Move command handlers into focused modules.

## Quick Hygiene

### 4. Delete unused skill `cursor-guide` (H1 · High · quick_win)
- Hypothesis: The skill is on disk but has not fired in the lookback window.
- Evidence: 0 invocations in 30 days · last edit 78 days ago
- Saves ~56000 tokens/week (est.) · Confidence: High
- Effort: 5 min · Apply: rm -rf .cursor/skills/cursor-guide

### 5. Route mechanical work away from premium models (H7 · Medium · hygiene)
- Hypothesis: Low-cost, repeated sessions still use a premium model name.
- Evidence: 12 sessions · median session 8 min
- Saves ~4800 tokens/week (est.) · Confidence: Medium
- Effort: 25 min · Apply: Set a cheaper default model for routine edit loops.

## Raw Stats

| Metric | Value |
|---|---|
| Sessions | 9 |
| Total cost | $12.40 |
| Top model | claude-sonnet (62%) |
| Top tool | read_file (38%) |
| Median session | 14 min |
```

## Install

You need **Rust 1.95+** from [rustup](https://rustup.rs).

```bash
cargo install kaizen-cli --locked
```

The command installs the `kaizen` binary into `~/.cargo/bin` or
`$CARGO_HOME/bin`. If your shell cannot find it, add that directory to
`PATH` and open a new terminal.

Developing Kaizen itself? Use `./scripts/install-local.sh` from a clone,
or `cargo install --path . --locked`. Detailed install notes live in
[docs/install.md](docs/install.md).

## Quick Start

```bash
cargo install kaizen-cli --locked
cd my-repo
kaizen init
# use your coding agent for a day...
kaizen summary
kaizen retro
```

`kaizen init` creates local storage under `.kaizen/` and wires supported
agent hooks idempotently. Re-running it is safe; originals back up under
`.kaizen/backup/`.

## How the Loop Works

**Observe.** Kaizen tails agent transcripts and hook events into
workspace-local SQLite. It can watch Cursor, Claude Code, Codex, OpenClaw,
Goose, OpenCode, and Copilot without running the model itself.

**Summarise.** `kaizen summary` and `kaizen metrics` roll sessions up by
agent, model, cost, tool use, and repo facts. The metrics pass also indexes
file-level and graph facts so retros can talk about this codebase, not only
token totals.

**Propose.** `kaizen retro` runs deterministic heuristics and groups bets by
confidence and action type: one high-confidence bet, up to two investigations,
and up to two quick hygiene fixes.

**Measure.** `kaizen exp new` can bind a change to git history and compare
control vs treatment sessions with bootstrap confidence intervals.

Nothing leaves disk unless you configure sync or a provider query. Team paths
redact secrets, env vars, absolute paths, and git emails before upload.

## Why Kaizen

| You want... | Existing tool | Kaizen |
|---|---|---|
| Cost per session for Claude Code | `ccusage`, `claude-usage-report` | Yes, plus Cursor, Codex, OpenClaw, and hook provenance |
| Cost per session for Cursor | Manual transcript work | Best-effort token and model recovery from transcript tails |
| One local view across agents | Glue scripts | Unified store, one CLI, one MCP surface |
| Repo-aware improvement bets | Dashboards only | Weekly retro with evidence, confidence, and apply steps |
| Local-first data | Hosted account | SQLite by default; sync is opt-in and redacted |
| Measure whether a fix worked | Spreadsheets | Git-bound A/B experiments and bootstrap reports |

Kaizen is not a dashboard. It is an opinionated feedback loop:
**capture → summarise → propose change → measure**.

## Advanced Features

- **HTTP proxy:** `kaizen proxy run` logs Anthropic API calls with precise
  token counts and optional context policy trimming.
- **Provider queries:** PostHog and Datadog rollups can be read with
  `--source provider` or merged with local rows via `--source mixed` when
  `[telemetry.query]` is configured.
- **Redacted sync:** shared endpoints receive batches only after local
  redaction and UUIDv7 deduplication.
- **A/B experiments:** `kaizen exp new --bind git` classifies later sessions
  by commit boundary and reports treatment deltas.
- **MCP tools:** most read/report commands are exposed to agents; shell-only
  commands stay in the CLI.
- **Local depth:** optional test/lint snapshots and CPU/RSS samples improve
  retro signals without shipping raw command output off disk.

## Docs

| Doc | Purpose |
|---|---|
| [docs/install.md](docs/install.md) | Install, build from source, uninstall |
| [docs/tutorial/README.md](docs/tutorial/README.md) | Hands-on tutorial |
| [docs/usage.md](docs/usage.md) | CLI reference |
| [docs/concepts.md](docs/concepts.md) | Sessions, events, retro, experiments |
| [docs/retro.md](docs/retro.md) | Heuristic retro engine |
| [docs/experiments.md](docs/experiments.md) | A/B experiment workflow |
| [docs/architecture.md](docs/architecture.md) | Module graph, data flow |
| [docs/config.md](docs/config.md) | Config file and env vars |
| [docs/telemetry-journey.md](docs/telemetry-journey.md) | How sessions become stored facts |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Dev setup, tests, PR flow |
| [CHANGELOG.md](CHANGELOG.md) | Release notes |

## Status

Pre-1.0: breaking changes may appear in minor versions. Follow
[CHANGELOG.md](CHANGELOG.md) for release notes.

## License

[AGPL-3.0-or-later](LICENSE). Contributions are licensed under the same
terms. Security disclosures: [SECURITY.md](SECURITY.md).
