# kaizen

Kaizen captures every coding agent session — Cursor, Claude Code, Codex — into a local SQLite database, then closes the feedback loop that most observability tools skip: a **heuristic retro engine** that ranks concrete improvement bets by tokens-saved-per-effort, and an **A/B experiment framework** that measures whether each bet worked. Nothing leaves disk until you say so.

Narrative guides and references live in this repository under [`docs/`](docs/README.md). The
**CLI** is published on [crates.io](https://crates.io/crates/kaizen-cli) as **`kaizen-cli`**. 
Install with **`cargo install kaizen-cli --locked`**, or
[build from a git clone](#install) if you are developing the project. The **Rust API** is on
[docs.rs/kaizen-cli](https://docs.rs/kaizen-cli). Long-form markdown stays in this repo. See
[Install](#install) for `PATH`, Homebrew, and troubleshooting.

[![crates.io](https://img.shields.io/crates/v/kaizen-cli.svg)](https://crates.io/crates/kaizen-cli)
[![CI](https://github.com/marquesds/kaizen/actions/workflows/ci.yml/badge.svg)](https://github.com/marquesds/kaizen/actions/workflows/ci.yml)
[![License: AGPL v3](https://img.shields.io/badge/License-AGPL_v3-blue.svg)](LICENSE)
[![Sponsor](https://img.shields.io/badge/Sponsor-FF5E5B?logo=ko-fi&logoColor=white)](https://ko-fi.com/lucasmarques)

## Agile retrospectives for coding agents

Agents are opaque. They burn tokens on files you didn't expect, loop on module boundaries you haven't mapped, and load skills that never fire. Most tools show you what happened. Kaizen tells you what to change — and proves whether the change worked.

The loop is **observe → summarise → propose → measure**. Each step is a real command.

**Observe** across three ingest tiers, zero agent restarts. `kaizen init` wires transcript tails (file notifications on agent JSONL directories) and hooks (`.cursor/hooks.json`, `.claude/settings.json`). The optional **LLM HTTP proxy** goes further: run `kaizen proxy run`, set `ANTHROPIC_BASE_URL=http://127.0.0.1:3847`, and every Anthropic API call is logged with precise token counts — no changes to the agent. The proxy optionally applies a **context policy** (`last_messages: 20` or `max_input_tokens: 200000`) that trims billed context before requests leave your machine.

**Summarise** at the repository level, not just the token level. Sessions and tool spans accumulate in a local SQLite WAL. The metrics pass walks git and your source tree to build a **code graph** (`file_facts`, `repo_edges`), so retros and experiments can answer: which files co-appear in long sessions, which module boundaries cause agent edit loops, which skills are loaded every turn but never triggered.

**Propose** with 14 deterministic heuristics, no LLM required. `kaizen retro --days 7` ranks bets by `tokens_saved_per_week / effort_minutes`:

```
### 1. Delete unused skill `cursor-guide` (H1)
- Saves ~56k tokens/week (~$0.84)
- Evidence: 0 invocations in 30d, last edit 78d ago
- Apply: rm -rf .cursor/skills/cursor-guide
- Effort: 2 min
```

Deterministic, formally specced in Quint, cheap to run on any schedule. The same engine ships as an **agent skill**: ask *"what should I improve?"* mid-session and kaizen surfaces the top bets inline without leaving your editor.

**Measure** with bootstrap statistics. `kaizen exp new --bind git` ties a hypothesis to a git commit boundary. Kaizen auto-classifies every subsequent session as control or treatment by walking `git log`. After the window closes, `kaizen exp report` prints:

```
Metric: tokens_per_session
         N    median
control  41   18,420
treatment 38  15,902

Delta: −2,518 tokens (−13.7%)
95% bootstrap CI: [−4,102, −1,030]   Target: −10% → MET
```

Non-parametric, 10k resamples, winsorized at p1/p99. Works for skill additions, rule changes, and architecture refactors — anything you can pin to a commit.

**Distribute** with redact-first sync. Configure a shared team endpoint and kaizen ships redacted batches: Aho-Corasick secret scanning, env var stripping, absolute path normalization, and git email removal run on every event before it leaves disk. The redaction model is formally verified in a Quint spec. Sync is opt-in, idempotent (UUIDv7 dedup), and restartable after failures.

## Why

- **Cost visibility** — tokens and USD per session, per model, per agent.
- **Session history** — searchable, live-tailable, across all three agents.
- **Heuristic retro** — weekly bets: what to change to make agents cheaper / faster.
- **Experiments** — A/B a rule, skill, or repo change against a real metric.
- **Fits your topology** — self-host, laptop, or a shared place your team can use; tailable session streams; you decide when anything syncs and **redact first**.

## Why kaizen over the alternatives

| You want… | Existing tool | Kaizen |
|---|---|---|
| Cost per session for **Claude Code** | `ccusage`, `claude-usage-report` | ✅ plus Cursor + Codex + hook provenance |
| Cost per session for **Cursor** | none (transcripts strip usage) | ✅ best-effort token + model from transcript tail |
| One pane of glass across agents | glue scripts | ✅ unified store, one CLI, one MCP |
| Turn observations into change | dashboards only | ✅ weekly heuristic **retro** + **experiments** (A/B) |
| Self-host, not locked to a vendor cloud | needs an account | ✅ deploy the binary, tail agent sessions live, SQLite + optional **redacted** sync |
| Ship MCP tools to agents | depends | ✅ most commands as MCP tools; shell-only for doctor, guidance, gc, completions, proxy, telemetry |
| Rust, single static binary, sub‑second cold start | varies | ✅ build from a checkout (`cargo install --path .` or `./scripts/install-local.sh`) |

Kaizen is not a dashboard — it is an opinionated feedback loop: **capture → summarise → propose change → measure**. Start with `kaizen init` in any repo where you use a coding agent.

## How it works (about 60 seconds)

Kaizen does not run the model. It **observes** agent activity: conversation and tool use land in
local SQLite; optional HTTP proxy logging adds another path. A metrics pass ties sessions to
**file-level** and **graph** facts so the CLI, TUI, retro, and experiments can reason about your
**repo**, not just token totals. Read the full pipeline (with a diagram) in
[docs/telemetry-journey.md](docs/telemetry-journey.md).

| If you want… | Start here |
|-------------|------------|
| Cost and rollups by agent / model | [docs/usage.md](docs/usage.md) (`summary`, `metrics`) |
| Browse and tail sessions | [docs/usage.md](docs/usage.md) (`sessions`, `tui`) |
| Heuristic “what to change” weekly bets | [docs/retro.md](docs/retro.md) |
| A/B a rule or change against a metric | [docs/experiments.md](docs/experiments.md) |
| The end-to-end data story (ingest → store → facts) | [docs/telemetry-journey.md](docs/telemetry-journey.md) |
| Hands-on tutorial (all features, exercises) | [docs/tutorial/README.md](docs/tutorial/README.md) |

## Demo


https://github.com/user-attachments/assets/3cf4ac40-cef7-480a-9bea-af69df06f3c6



## Install

You need **Rust 1.95+** ([rustup](https://rustup.rs)). **Git** is only required for a source build.

Install the `kaizen` binary from **crates.io** with [`cargo install kaizen-cli --locked`](https://crates.io/crates/kaizen-cli) (step 1). To **build from a clone** of this repo instead, use step 2.

1. **Install the CLI from [crates.io](https://crates.io/crates/kaizen-cli)** (writes `kaizen` to
   `~/.cargo/bin`, or `$CARGO_HOME/bin`):

   ```bash
   cargo install kaizen-cli --locked
   ```

   **Or use Homebrew** from a [tap](https://docs.brew.sh/Taps) once you publish
   [`packaging/homebrew/kaizen-cli.rb`](packaging/homebrew/kaizen-cli.rb) to a `homebrew-tap` repo:
   `brew tap <user>/tap && brew install kaizen-cli` (installs the same `kaizen` binary). See
   [docs/install.md](docs/install.md#from-homebrew-third-party-tap).

2. **Or build from a git clone** (for contributors and `--path` installs):

   ```bash
   git clone https://github.com/marquesds/kaizen.git
   cd kaizen
   ./scripts/install-local.sh
   ```

   Equivalent: `cargo install --path . --locked` from the repo root (use `--force` to replace an
   existing install).

3. **Confirm `kaizen` is on your `PATH`** (rustup usually adds `~/.cargo/bin`). If the shell
   cannot find `kaizen`, add that directory to `PATH` and open a new terminal.

4. **Run `kaizen` in your own project** (where you use Cursor / Claude Code / Codex), not only
   inside the kaizen repo:

   ```bash
   cd /path/to/your-project
   kaizen init
   ```

Step-by-step guide, uninstall, and troubleshooting: [docs/install.md](docs/install.md).

## Quick start

```bash
kaizen init                  # scaffold .kaizen/ + wire .cursor/hooks.json + .claude/settings.json
kaizen doctor                # verify config, DB, and hook wiring
kaizen sessions list         # index sessions in the current repo
kaizen summary               # cost / agent / model rollup
kaizen tui                   # live session browser
kaizen retro --days 7        # weekly heuristic bets
kaizen exp new --name add-skill \
  --hypothesis "skill cuts tokens" --change "add .cursor/skills/x" \
  --metric tokens_per_session --bind git \
  --duration-days 14 --target-pct=-10
```

`kaizen init` creates both hook files when absent and patches them idempotently when present. Re-running is safe; originals back up under `.kaizen/backup/`.

Full CLI reference: [docs/usage.md](docs/usage.md). Guided walkthrough: [docs/tutorial/README.md](docs/tutorial/README.md).

## Docs

| Doc | Purpose |
|---|---|
| [docs/install.md](docs/install.md) | Install, build from source, uninstall |
| [docs/tutorial/README.md](docs/tutorial/README.md) | Hands-on tutorial (all major features) |
| [docs/usage.md](docs/usage.md) | CLI reference |
| [docs/concepts.md](docs/concepts.md) | Sessions, events, retro, experiments |
| [docs/architecture.md](docs/architecture.md) | Module graph, data flow |
| [docs/config.md](docs/config.md) | Config file + env vars |
| [docs/telemetry-journey.md](docs/telemetry-journey.md) | How sessions become stored facts (learning path) |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Dev setup, tests, PR flow |
| [CHANGELOG.md](CHANGELOG.md) | Release notes |

## Status

Pre–1.0: breaking changes may appear in minor versions (see
[CHANGELOG.md](CHANGELOG.md)). Feature set for the initial public line is
shipped; follow the changelog for new work.

## License

[AGPL-3.0-or-later](LICENSE). Contributions are licensed under the same
terms. See [CONTRIBUTING.md](CONTRIBUTING.md).

Security disclosures: [SECURITY.md](SECURITY.md).
