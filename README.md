# kaizen

Local-first telemetry and tooling for AI coding agent sessions
(Cursor, Claude Code, Codex). Collect, store, and reason about what
agents do in your repos — offline by default, redacted before any sync.

[![CI](https://github.com/lucasmarqs/kaizen/actions/workflows/ci.yml/badge.svg)](https://github.com/lucasmarqs/kaizen/actions/workflows/ci.yml)
[![License: AGPL v3](https://img.shields.io/badge/License-AGPL_v3-blue.svg)](LICENSE)
[![Crates.io](https://img.shields.io/crates/v/kaizen.svg)](https://crates.io/crates/kaizen)

## Why

- **Cost visibility** — tokens and USD per session, per model, per agent.
- **Session history** — searchable, live-tailable, across all three agents.
- **Heuristic retro** — weekly bets: what to change to make agents cheaper / faster.
- **Experiments** — A/B a rule, skill, or repo change against a real metric.
- **Local-first** — SQLite on your disk. No data leaves the machine unless you configure sync.

## Install

```bash
cargo install kaizen
kaizen init
```

Requires Rust 1.85+ (edition 2024). Full install guide:
[docs/install.md](docs/install.md).

## Quick start

```bash
kaizen init                  # scaffold .kaizen/ + wire hooks
kaizen sessions list         # index sessions in the current repo
kaizen summary               # cost / agent / model rollup
kaizen tui                   # live session browser
kaizen retro --days 7        # weekly heuristic bets
kaizen exp new --name add-skill \
  --hypothesis "skill cuts tokens" --change "add .cursor/skills/x" \
  --metric tokens_per_session --bind git \
  --duration-days 14 --target-pct -10
```

Full CLI reference: [docs/usage.md](docs/usage.md).

## Docs

| Doc | Purpose |
|---|---|
| [docs/install.md](docs/install.md) | Install, build from source, uninstall |
| [docs/usage.md](docs/usage.md) | CLI reference |
| [docs/concepts.md](docs/concepts.md) | Sessions, events, retro, experiments |
| [docs/architecture.md](docs/architecture.md) | Module graph, data flow |
| [docs/config.md](docs/config.md) | Config file + env vars |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Dev setup, tests, PR flow |
| [ROADMAP.md](ROADMAP.md) | Milestone status |
| [CHANGELOG.md](CHANGELOG.md) | Release notes |

## Status

Pre–v0.1. APIs evolve. See [ROADMAP.md](ROADMAP.md) and
[CHANGELOG.md](CHANGELOG.md).

## License

[AGPL-3.0-or-later](LICENSE). Contributions are licensed under the same
terms. See [CONTRIBUTING.md](CONTRIBUTING.md).

Security disclosures: [SECURITY.md](SECURITY.md).
