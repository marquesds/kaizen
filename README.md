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

## Quick start (from source)

Requires a **Rust 2024** toolchain (edition 2024; use a current stable or nightly
that supports it—check `edition` in `Cargo.toml`).

```bash
cargo build --release
./target/release/kaizen --help
```

`cargo install` from crates.io is planned for a stable release; until then,
install with `cargo install --path .` from a clone.

## Status

Pre–v0.1: APIs and features evolve. See [ROADMAP.md](ROADMAP.md).

## CI

GitHub Actions runs `fmt`, `clippy` (`-D warnings`), and `cargo test` on
push/PR. Workflow: [`.github/workflows/ci.yml`](.github/workflows/ci.yml).
Add a [status badge](https://docs.github.com/en/actions/monitoring-and-troubleshooting-workflows/adding-a-workflow-status-badge) after the repo URL is final.
