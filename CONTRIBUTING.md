# Contributing

- **Build / check:** `cargo test && cargo clippy -- -D warnings && cargo fmt --all -- --check`
- **Milestones and task order:** [docs/impl-sequence.md](docs/impl-sequence.md) and [ROADMAP.md](ROADMAP.md)
- **Local-only folders:** a `discoveries/` directory at the repo root (if
  you use it for spikes or notes) is **gitignored** and must not be relied
  on in docs or code paths shipped to others.

For coding standards, see [AGENTS.md](AGENTS.md) and [docs/patterns.md](docs/patterns.md).
