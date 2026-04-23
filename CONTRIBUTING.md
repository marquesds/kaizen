# Contributing

Thanks for considering a contribution. This project ships under
[AGPL-3.0-or-later](LICENSE); by opening a PR you agree your work is
licensed under the same terms.

Please read the [Code of Conduct](CODE_OF_CONDUCT.md) before engaging.

## Getting started

```bash
git clone https://github.com/lucasmarqs/kaizen
cd kaizen
cargo build
cargo test
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

CI enforces all of the above on push and PR. See
`.github/workflows/ci.yml`.

## Branch + PR flow

1. Fork + branch from `main`.
2. Small, focused commits. Conventional Commits preferred:
   `feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`.
3. Update docs and `CHANGELOG.md` (Unreleased section) when user-visible
   behavior changes.
4. Open PR; fill in the template.
5. A maintainer reviews and merges.

## Tests

- Unit tests live next to the code.
- Integration tests in `tests/`.
- Spec tests in `tests/spec/` drive Quint state machines from
  `specs/*.qnt` via `quint-connect`.
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

- **GitHub:** the `docs/` tree in this repository is the home for long-form documentation.
- **crates.io / docs.rs:** the published package documents the **Rust API**; it does not include
  the `docs/` markdown (see `exclude` in `Cargo.toml`).

## Versioning

[SemVer](https://semver.org). Pre-`1.0` breaking changes may land in
minor versions; document in `CHANGELOG.md`.

## Releasing (maintainers only)

The workflow is [`.github/workflows/release.yml`](.github/workflows/release.yml)
(Linux and macOS binaries, GitHub Release, and crates.io for stable version
names — no `-[prerelease]` suffix in the SemVer). Local `cargo login` is not
used in CI. Add a [crates.io API token](https://doc.rust-lang.org/cargo/reference/publishing.html)
with `publish` scope to the repository secret **`CARGO_REGISTRY_TOKEN`**.

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

## Security

Report vulnerabilities per [SECURITY.md](SECURITY.md) — do not open
public issues for security bugs.

## Agent rules

If you use AI agents to contribute, read [AGENTS.md](AGENTS.md) first.
Same bar applies to agent-assisted code: fmt, clippy, tests, docs.
