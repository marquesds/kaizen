# kaizen

Local-first telemetry and tooling for **AI coding agent** sessions
(Cursor, Claude Code, Codex): collect, store, and reason about what agents
do in your repos.

**License:** [GNU Affero General Public License v3.0](LICENSE) (AGPL-3.0).
By contributing, you agree your contributions are under the same license.

## Docs

| Doc | Purpose |
|-----|---------|
| [docs/README.md](docs/README.md) | Index of all design docs |
| [ROADMAP.md](ROADMAP.md) | Milestone status (M0–M7) |
| [docs/impl-sequence.md](docs/impl-sequence.md) | Build order and done-signals |
| [docs/roadmap.md](docs/roadmap.md) | Risks, v0.1 scope, definition of done |
| [CONTRIBUTING.md](CONTRIBUTING.md) | How to work on the crate |

## Quick start

```bash
cargo install kaizen
kaizen init                        # scaffold .kaizen/ + wire hooks
kaizen sessions list               # index + list sessions
kaizen summary                     # cost/agent/model rollup
kaizen retro --days 7              # weekly bets report
kaizen tui                         # live session browser
kaizen exp new --name add-skill \
  --metric tokens_per_session --bind git \
  --duration-days 14 --target-pct -10
```

From a clone: `cargo install --path .` (edition 2024; use current
stable Rust or nightly that supports it).

## Status

Pre–v0.1: APIs and features evolve. See [ROADMAP.md](ROADMAP.md).

## CI

GitHub Actions runs `fmt`, `clippy` (`-D warnings`), `cargo test`, and
Quint spec tests on push/PR. Separate job runs `cargo audit` and
`cargo deny check` against `.cargo/deny.toml`.
Workflow: [`.github/workflows/ci.yml`](.github/workflows/ci.yml).
Add a [status badge](https://docs.github.com/en/actions/monitoring-and-troubleshooting-workflows/adding-a-workflow-status-badge) after the repo URL is final.

## Release

Maintainer-only: `scripts/release.sh <version>` runs pre-flight checks,
bumps `Cargo.toml`, cross-compiles for macOS + Linux, and publishes to
crates.io. Requires `cargo-edit`, `cross`, and a crates.io token.
