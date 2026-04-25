# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Pre-`1.0`: breaking changes may land in minor versions and will be called out
here explicitly.

## [Unreleased]

### Added

- **OpenClaw integration** — full tail + hook support for [OpenClaw](https://openclaw.ai) (local
  AI gateway, multi-provider). `kaizen init` writes
  `~/.openclaw/hooks/kaizen-events/handler.ts` (TS webhook handler) and backs up any prior
  handler. `kaizen doctor` reports hook wiring. `kaizen sessions list` / `summary` include
  OpenClaw sessions. Workspace filter mirrors the OpenCode strategy: sessions are admitted only
  when a tool-call payload contains a `cwd` / `directory` / `projectPath` field matching the
  current repo. Channel metadata (`dm`, `slack`, etc.) is stored as `meta.channel` on every
  event. Pricing: Anthropic and OpenAI models use their respective named cost rows; local
  models fall back to the new `openclaw` heuristic row (Sonnet-scale, 5 000 avg tokens/turn).
  New config toggle: `sources.tail.openclaw` (default `true`). New env vars:
  `OPENCLAW_STATE_DIR`, `OPENCLAW_HOME`. Formally modelled in
  `specs/openclaw-ingest.qnt` (workspace filter invariants); `specs/hook-ingest.qnt` and
  `specs/init-setup.qnt` extended with openclaw as a fifth hook-host slot.

### Changed

- **Machine-local project registry** lives in `~/.kaizen/machine.db` (SQLite) instead of `workspaces.json`; `kaizen init` upserts the current repo. Legacy `workspaces.json` is imported once and renamed to `workspaces.json.migrated`. `kaizen doctor` reports registry status. `--all-workspaces` still merges per-repo stores and now includes inited projects that do not yet have `.kaizen/kaizen.db`.
- **Telemetry wire format (third-party sinks):** PostHog capture is one event per canonical row (`kaizen.event`, `kaizen.tool_span`, `kaizen.repo_snapshot_chunk`, `kaizen.workspace_fact_snapshot`); Datadog uses the [Logs API v2](https://docs.datadoghq.com/api/latest/logs/) (`POST /api/v2/logs`) with one JSON log per canonical item instead of the Events API. OTLP remains a placeholder with `tracing::debug` of expanded item counts. Primary Kaizen `POST` ingest and outbox JSON shapes for `events` / `tool_spans` / `repo_snapshots` are unchanged; when sync is enabled, workspace skill/rule discovery can enqueue **`workspace_facts`** for the new `/v1/workspace-facts` path.
- **CLI — read paths and telemetry:** `summary`, `insights`, `metrics`, `guidance`, and `retro` accept `--source local|provider|mixed` (default `local`). With `provider` or `mixed`, a background provider pull runs when `[telemetry.query].cache_ttl_seconds` has expired, or when you pass `--refresh` (in addition to transcript rescan where applicable). New `kaizen telemetry` subcommands: `init` (alias of `configure`), `doctor`, `pull --days`, and `print-schema`. MCP tools keep the previous local-only behavior.
- `Cargo.toml` no longer excludes `assets/` so `cargo publish` / docs.rs builds resolve
  `include_str!` for embedded defaults and the retro skill template.
- Release workflow **`update-homebrew-tap`**: `scripts/render-homebrew-tap-formula.sh` + push to
  **marquesds/homebrew-tap** when `HOMEBREW_TAP_TOKEN` is set.
- The crates.io / `cargo install` **package** name is **`kaizen-cli`** (the unscoped `kaizen` crate on
  the registry is unrelated); the `[[bin]]` and library names stay **`kaizen`**. README,
  `docs/install.md`, and `CONTRIBUTING` document `cargo install kaizen-cli` and, under
  `packaging/homebrew/`, a sample tap formula and [`packaging/homebrew/README.md`](packaging/homebrew/README.md)
  for Homebrew. `Cargo.toml` `repository` / `homepage` use `https://github.com/marquesds/kaizen`;
  `CONTRIBUTING` still documents `CARGO_REGISTRY_TOKEN`, `RELEASE_PUSH_TOKEN`, and optional
  `HOMEBREW_TAP_TOKEN`.

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
- Release workflow: green CI on `main` tags `vX.Y.Z` from `Cargo.toml` when that tag is free, otherwise **patch-bumps** (e.g. `v0.1.0` → `v0.1.1`) until a free semver tag and triggers the Release workflow. `cargo publish` runs only when the tag equals `Cargo.toml` (same patch); additional patch tags ship GitHub release binaries only.

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

[Unreleased]: https://github.com/marquesds/kaizen/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/marquesds/kaizen/releases/tag/v0.1.0
