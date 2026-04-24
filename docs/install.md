# Install

The registry package is **[`kaizen-cli`](https://crates.io/crates/kaizen-cli)**; `cargo install kaizen-cli`
places the **`kaizen`** binary on your `PATH`. Prefer `cargo install kaizen-cli --locked` for released
versions; use a **git clone** when you need `main` or are contributing. Long-form user docs live in
the [GitHub `docs/`](https://github.com/marquesds/kaizen/tree/main/docs) tree, not in the registry package tarball.

## Requirements

- **Rust 1.95+** (edition 2024). Install with [rustup](https://rustup.rs), then `rustup update stable`.
- **Git** for a source install or development (not required for `cargo install` from crates.io).
- **macOS or Linux.** Windows is not supported in v0.1.
- Optional (contributors): Node 20+ for Quint specs, `cargo-audit`, `cargo-deny`.

## From crates.io

**Recommended for end users** — installs a released version into Cargo’s bin directory
(`~/.cargo/bin/kaizen`, or `$CARGO_HOME/bin/kaizen`):

```bash
cargo install kaizen-cli --locked
```

- Pin a version if you need a specific line: `cargo install kaizen-cli --locked --version 0.1.0`, or
  install from a git revision (see the [cargo install](https://doc.rust-lang.org/cargo/commands/cargo-install.html#installing-a-git-repository) book section).

The unrelated [`kaizen`](https://crates.io/crates/kaizen) crate on crates.io is a different project; this repository publishes as **`kaizen-cli`** only.

## From Homebrew (third-party tap)

There is no [homebrew-core](https://github.com/Homebrew/homebrew-core) formula in this repository
yet. Install from a **tap** that hosts the formula (for example your own
[`homebrew-tap`](https://github.com/marquesds/homebrew-tap) after you copy
[`packaging/homebrew/kaizen-cli.rb`](https://github.com/marquesds/kaizen/blob/main/packaging/homebrew/kaizen-cli.rb)
and fill in `sha256` from the release assets). Maintainer steps: [packaging/homebrew/README.md](https://github.com/marquesds/kaizen/blob/main/packaging/homebrew/README.md).

```bash
brew tap marquesds/tap
brew install kaizen-cli
```

This installs the **`kaizen`** binary (formula name `kaizen-cli`). Uninstall: `brew uninstall kaizen-cli`.

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

## Update or reinstall

**From crates.io** (picks latest compatible release, or set `--version`):

```bash
cargo install kaizen-cli --locked --force
```

**From the same git clone** — after `git pull` (or local edits), reinstall from the repo root:

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
cargo uninstall kaizen-cli
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
