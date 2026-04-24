# Contributing

Thanks for considering a contribution. This project ships under
[AGPL-3.0-or-later](LICENSE); by opening a PR you agree your work is
licensed under the same terms.

Please read the [Code of Conduct](CODE_OF_CONDUCT.md) before engaging.

## Getting started

```bash
git clone https://github.com/marquesds/kaizen
cd kaizen
cargo build
cargo test
```

Install the binary from your working tree into `~/.cargo/bin` (handy after local changes):

```bash
./scripts/install-local.sh
```

Requires Rust 1.95+ (edition 2024). For Quint specs:

```bash
npm install -g @informalsystems/quint@0.32.0
```

For release / audit:

```bash
cargo install --locked cargo-audit cargo-deny cargo-edit cross
```

## Checks

Every PR must pass:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
cargo deny --manifest-path Cargo.toml check --config .cargo/deny.toml
```

After changing `specs/*.qnt`, run (same checks CI runs once Quint is installed):

```bash
scripts/check-quint-specs.sh
```

CI enforces all of the above on push and PR. See
`.github/workflows/ci.yml`.

On GitHub, protect `main`: require pull requests, and require every job in the
CI workflow (including **test (macos-latest / stable)**) to pass before merge.
That stops Linux-green / macOS-red changes from landing when someone bypasses
local runs.

## Branch + PR flow

1. Fork + branch from `main`.
2. Small, focused commits. Conventional Commits preferred:
   `feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`.
3. Update docs and `CHANGELOG.md` (Unreleased section) when user-visible
   behavior changes.
4. Open PR; fill in the template.
5. A maintainer reviews and merges.

Avoid pushing directly to `main`; branch protection should block it so CI always
runs on a PR first.

## Tests

- Unit tests live next to the code.
- Integration tests in `tests/`.
- Spec tests in `tests/spec/` drive Quint state machines from
  `specs/*.qnt` via `quint-connect`. Coverage map:
  [docs/quint-coverage.md](docs/quint-coverage.md).
- Fixtures in `tests/fixtures/`.

## Docs

### Documentation expectations

- **Reader-facing** files (`docs/**/*.md` user guides, root `README.md`, this file, and
  user-oriented `CHANGELOG` entries) should use full, clear technical prose. Prefer short
  sections, concrete examples, and honest limits (what is exact vs heuristic).
- **Internal** agent rules, skills, and specs stay in their own style; see
  [AGENTS.md](AGENTS.md).

The index in [docs/README.md](docs/README.md) includes a **“Keep docs current”** table: when you
change behavior or data flow, update the listed documents in the same PR when practical.

### Where docs are published

- **GitHub:** the `docs/` tree in this repository is the home for long-form user documentation.
- **crates.io / docs.rs:** the package [`kaizen-cli`](https://crates.io/crates/kaizen-cli) documents the
  **Rust API** on [docs.rs/kaizen-cli](https://docs.rs/kaizen-cli). The `docs/` markdown book is not in the
  registry tarball (see `exclude` in `Cargo.toml`). The on-disk CLI binary remains **`kaizen`**.

## Versioning

[SemVer](https://semver.org). Pre-`1.0` breaking changes may land in
minor versions; document in `CHANGELOG.md`.

## Releasing (maintainers only)

The workflow is [`.github/workflows/release.yml`](.github/workflows/release.yml)
(Linux and macOS binaries, GitHub Release, and crates.io for stable version
names — no `-[prerelease]` suffix in the SemVer). Local `cargo login` is not
used in CI.

### Required GitHub Actions secrets

Configure these under **Settings → Secrets and variables → Actions** (or
organization-level equivalents):

| Secret | Purpose |
|--------|---------|
| **`CARGO_REGISTRY_TOKEN`** | [crates.io API token](https://doc.rust-lang.org/cargo/reference/publishing.html) with **publish** scope. Without it, the `cargo publish` job fails. |
| **`RELEASE_PUSH_TOKEN`** | PAT used to create GitHub Releases and tags from the workflow (see the workflow’s `softprops/action-gh-release` steps). |

### First `cargo publish` to crates.io

- This repository publishes the **package name `kaizen-cli`** (the [`kaizen`](https://crates.io/crates/kaizen)
  name on crates.io is a different, third-party crate). `cargo publish` publishes `kaizen-cli`.
- Pushing a stable tag that **exactly** matches `version` in `Cargo.toml` (e.g. `v0.1.0` and
  `0.1.0`) is what triggers `cargo publish` (see the `publish` job’s `if:` in the workflow file).

**Path A (GitHub — creates tag in CI).** On `main`, merge a commit that
bumps `version` in `Cargo.toml` and adds a `## [X.Y.Z]` section in
`CHANGELOG.md` (or a prerelease version string if applicable). In GitHub:
Actions → **Release** → **Run workflow** → enter the same version as in
`Cargo.toml` (with or without a leading `v`). The run verifies the value,
builds, creates the `vX.Y.Z` **tag** and **GitHub Release** at the workflow
**commit** (`target_commitish`), and runs **`cargo publish`** when the
version is a stable (non-prerelease) release.

**Path B (local signed tag).** [scripts/release.sh](scripts/release.sh) runs
checks, bumps the version, commits, creates a **signed** `vX.Y.Z` tag, and
pushes. Pushing the tag still triggers the same workflow; use this when you
need GPG-signed tags.

**Path C (manual tag on `main`).** If you already merged the release commit
and only need to trigger the workflow, ensure the tag matches `version` in
`Cargo.toml` (e.g. `0.1.0` → `v0.1.0`):

```bash
git switch main && git pull
git tag v0.1.0
git push origin v0.1.0
```

Skip the manual `git tag` if you used Path A (the workflow creates the tag).

## Security

Report vulnerabilities per [SECURITY.md](SECURITY.md) — do not open
public issues for security bugs.

## Agent rules

If you use AI agents to contribute, read [AGENTS.md](AGENTS.md) first.
Same bar applies to agent-assisted code: fmt, clippy, tests, docs.
