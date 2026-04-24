# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Pre-`1.0`: breaking changes may land in minor versions and will be called out
here explicitly.

## [Unreleased]

### Fixed

- `kaizen metrics` / `kaizen metrics index` returned `PARSE_ERROR` on `CONTAINS` because GraphQLite treats it as a reserved keyword; the codegraph now uses `HAS_FILE`.
- Repeated edges of the same kind between two files no longer abort the repo snapshot with a `UNIQUE constraint failed` error; duplicates now accumulate into a single edge weight.
- `kaizen ingest hook` now records the session `agent` as `cursor` / `claude` instead of literal `unknown`, falls back to the wall-clock timestamp when the hook payload omits `timestamp_ms` (Claude Code never sends one), and auto-provisions a session stub when the first observed event is a non-`SessionStart` (hooks installed mid-session).
- `kaizen guidance` no longer lists garbage skill / rule slugs like `\n`, `` ` ``, `{}`, `**` extracted from prose payloads; the path regex now accepts only real slug shapes, and legacy rows are filtered at query time.
- `kaizen exp new --target-pct -10` parses correctly; negative defaults in the docs no longer need `=`.

### Added

- `kaizen init` now creates `.cursor/hooks.json` and `.claude/settings.json` from scratch when absent (in addition to patching them when present), so a single command is enough to instrument a fresh workspace for both Cursor and Claude Code.
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
- Release workflow: first green CI on Cargo version `X.Y.Z` pushes exact tag `vX.Y.Z` and publishes crates.io; later green CI runs on the same Cargo version push prerelease tags like `vX.Y.Z-ci.<sha>` for GitHub Release binaries without re-publishing the crate.

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
