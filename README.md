# kaizen

Run and share **kaizen** so you can see how coding agents behave in
**real time**, run **retros** on that stream, and turn it into
**strategies to improve** your own repo. Unifies Cursor, Claude
Code, and Codex. One SQLite store and one CLI; **redact before** any
sync, and only sync on **your** terms.

Narrative guides and references live in this repository under [`docs/`](docs/README.md). The
crate is **not** on [crates.io](https://crates.io) yet, so install from a git checkout (see
[Install](#install)). When it is published, [docs.rs](https://docs.rs) will host the **Rust API**;
long-form markdown stays in this repo.

[![CI](https://github.com/marquesds/kaizen/actions/workflows/ci.yml/badge.svg)](https://github.com/marquesds/kaizen/actions/workflows/ci.yml)
[![License: AGPL v3](https://img.shields.io/badge/License-AGPL_v3-blue.svg)](LICENSE)

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
| Ship MCP tools to agents | depends | ✅ every CLI command surfaces as an MCP tool |
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

## Demo


https://github.com/user-attachments/assets/3cf4ac40-cef7-480a-9bea-af69df06f3c6



## Install

The CLI is **not** on crates.io yet. You need **Rust 1.95+** ([rustup](https://rustup.rs)) and
**git**.

1. **Clone and install the binary** (installs to `~/.cargo/bin`, or `$CARGO_HOME/bin`):

   ```bash
   git clone https://github.com/marquesds/kaizen.git
   cd kaizen
   ./scripts/install-local.sh
   ```

   Equivalent: `cargo install --path . --locked` from the repo root (use `--force` to replace an
   existing install).

2. **Confirm `kaizen` is on your `PATH`** (rustup usually adds `~/.cargo/bin`). If the shell
   cannot find `kaizen`, add that directory to `PATH` and open a new terminal.

3. **Run `kaizen` in your own project** (where you use Cursor / Claude Code / Codex), not only
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

Full CLI reference: [docs/usage.md](docs/usage.md).

## Docs

| Doc | Purpose |
|---|---|
| [docs/install.md](docs/install.md) | Install, build from source, uninstall |
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
