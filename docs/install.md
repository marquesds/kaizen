# Install

There is no crates.io package for this repository yet. You build the CLI from a **git clone**
and put `kaizen` on your `PATH` (see [From crates.io](#from-cratesio) for the future story).

## Requirements

- **Rust 1.95+** (edition 2024). Install with [rustup](https://rustup.rs), then `rustup update stable`.
- **Git** to clone the repository.
- **macOS or Linux.** Windows is not supported in v0.1.
- Optional (contributors): Node 20+ for Quint specs, `cargo-audit`, `cargo-deny`.

## Install from a git clone

**1. Clone the repository**

```bash
git clone https://github.com/marquesds/kaizen.git
cd kaizen
```

**2. Build and install the binary into Cargo’s bin directory**

Pick one:

| Command | When to use |
|--------|-------------|
| `./scripts/install-local.sh` | Recommended. Same as `cargo install --path . --locked --force`; prints where `kaizen` was installed or reminds you about `PATH`. |
| `cargo install --path . --locked` | Same install, without `--force` (fails if an old `kaizen` is already installed unless you pass `--force`). |

The binary is written to `~/.cargo/bin/kaizen`, or `$CARGO_HOME/bin/kaizen` if you set `CARGO_HOME`.

**3. Put Cargo’s bin directory on `PATH`**

[rustup](https://rustup.rs) usually adds `~/.cargo/bin` for you. If `kaizen` is not found:

```bash
command -v kaizen
# if empty, add to PATH (zsh/bash), then open a new shell:
export PATH="$HOME/.cargo/bin:$PATH"
```

**4. Use `kaizen` in a project**

`kaizen init` and the rest of the CLI are meant to run **inside a repo** where you use agents (not only inside the `kaizen` source tree):

```bash
cd /path/to/your-project
kaizen init
```

See [First run](#first-run) and [Verify](#verify).

### Without `cargo install` (local binary only)

To build without installing into `~/.cargo/bin`:

```bash
cargo build --release
./target/release/kaizen --version
# run with full path, or symlink `kaizen` somewhere on PATH
```

## From crates.io

**Not published yet.** Do not run `cargo install kaizen` for this project today; the `kaizen`
name on crates.io may point at a **different** crate. When this repository is published, this
section will document `cargo install kaizen`.

## Update or reinstall from the same clone

After `git pull` (or local edits), reinstall from the repo root:

```bash
./scripts/install-local.sh
```

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
kaizen doctor
kaizen sessions list
```

`kaizen doctor` checks the local store, config paths, and optional hook wiring.

## Shell completions (optional)

```bash
kaizen completions zsh
```

Redirect or eval per [docs/usage.md](usage.md) (`kaizen completions` section).

## Troubleshooting

- `error: edition2024 is unstable` — upgrade Rust: `rustup update stable`.
- `linker error` on Linux — install `build-essential` / `clang`.
- Empty `sessions list` — ensure the agent wrote transcripts; see
  [docs/concepts.md](concepts.md) for source paths.
