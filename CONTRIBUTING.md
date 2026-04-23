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

Requires Rust 1.85+ (edition 2024). For Quint specs:

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

Docs live in `docs/`. Keep them current — the table in
[docs/README.md](docs/README.md) lists what to update per change type.

## Versioning

[SemVer](https://semver.org). Pre-`1.0` breaking changes may land in
minor versions; document in `CHANGELOG.md`.

## Releasing (maintainers only)

See [scripts/release.sh](scripts/release.sh) and
`.github/workflows/release.yml`. Tagging `vX.Y.Z` triggers the release
workflow: cross-compile, attach binaries to the GitHub Release, publish
to crates.io.

## Security

Report vulnerabilities per [SECURITY.md](SECURITY.md) — do not open
public issues for security bugs.

## Agent rules

If you use AI agents to contribute, read [AGENTS.md](AGENTS.md) first.
Same bar applies to agent-assisted code: fmt, clippy, tests, docs.
