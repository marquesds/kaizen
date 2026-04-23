# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Pre-`1.0`: breaking changes may land in minor versions and will be called out
here explicitly.

## [Unreleased]

### Added

- Local retention: `[retention].hot_days` (default 30) prunes old sessions from SQLite after rescans (throttled to once per 24h); `hot_days = 0` disables auto-prune. `kaizen gc` with optional `--days` and `--vacuum`.
- `[scan].min_rescan_seconds` (default 300) skips full transcript rescans; `--refresh` / `-r` on `sessions list`, `summary`, `insights`, `metrics`, and `retro` forces a rescan. MCP tools accept `refresh=true` for the same behavior.
- Composite index `sessions(workspace, started_at_ms)` for faster session listing.
- `docs/telemetry-journey.md` — end-to-end “session → data” learning path; README and `docs/`
  index point to it. Root `README` clarifies that long-form docs live in the GitHub `docs/`
  tree, not in the crates.io package. Library `//!` doc links to `docs/`.
- `kaizen proxy run` — local HTTP forwarder for Anthropic-style APIs, `[proxy]` in `config.toml`, `docs/llm-proxy.md`.
- `docs/install.md`, `docs/usage.md`, `docs/concepts.md` — user-facing docs.
- `CODE_OF_CONDUCT.md`, `SECURITY.md`, issue + PR templates.
- `CHANGELOG.md` (this file).
- Release workflow: tag `vX.Y.Z` → cross-compile, GitHub Release, crates.io publish.

### Changed
- Config merge: `[retention]` and `[scan].min_rescan_seconds` now merge workspace + user TOML field-by-field (workspace first, then user overrides non-default fields). `[sources]` remains user-file-only.
- `AGENTS.md` and `.cursor/rules/caveman-writing.mdc` — reader-facing exception for `README.md`,
  `docs/**/*.md` user guides, `CONTRIBUTING.md`, and user-facing `CHANGELOG` entries.
- `Cargo.toml` crates.io metadata (description, license, repo, keywords, categories, `rust-version`, `exclude`).
- `README.md`, `docs/README.md`, `CONTRIBUTING.md` rewritten for OSS.

### Removed
- `ROADMAP.md` — use `docs/` and this file for feature scope; milestones from the former roadmap are shipped.
- `src/bin/spike_a.rs`, `src/bin/spike_e.rs` — done spike binaries.
- `docs/impl-sequence.md`, `docs/roadmap.md` — internal build-order docs.
- Stray `test_testSessionEndsInDone_0.itf.json` Quint trace.

## [0.1.0] — TBD

Initial public release. See `docs/usage.md` and the `[Unreleased]` section above for capabilities.

[Unreleased]: https://github.com/lucasmarqs/kaizen/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/lucasmarqs/kaizen/releases/tag/v0.1.0
