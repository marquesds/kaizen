# Install

## Requirements

- Rust 1.85+ (edition 2024). Install via [rustup](https://rustup.rs).
- macOS or Linux. Windows not supported in v0.1.
- Optional for dev: Node 20+ for Quint specs, `cargo-audit`, `cargo-deny`.

## From crates.io

```bash
cargo install kaizen
```

## From source

```bash
git clone https://github.com/lucasmarqs/kaizen
cd kaizen
cargo install --path .
```

Artifacts land in `~/.cargo/bin/kaizen`.

## First run

```bash
cd your-repo
kaizen init
```

`kaizen init` is idempotent. Writes `.kaizen/config.toml`, patches
agent hooks for Cursor / Claude Code, and installs the retro skill.
Safe to rerun.

## Uninstall

```bash
cargo uninstall kaizen
rm -rf ~/.kaizen .kaizen
```

Remove hook edits from `.cursor/hooks.json` and
`.claude/settings.json` if you want a full revert. `kaizen init`
backs up originals under `.kaizen/backup/`.

## Verify

```bash
kaizen --version
kaizen sessions list
```

## Troubleshooting

- `error: edition2024 is unstable` — upgrade Rust: `rustup update stable`.
- `linker error` on Linux — install `build-essential` / `clang`.
- Empty `sessions list` — ensure the agent wrote transcripts; see
  [docs/concepts.md](concepts.md) for source paths.
