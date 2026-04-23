# impl-sequence

Build order. Each item: what, key file, done-signal.
Cross-cutting: Quint spec first for state machines → Rust impl → `#[quint_run]` test.
Every PR: `cargo test && cargo clippy -D warnings && cargo fmt --check`.

Ref: [architecture](architecture.md) · [ingest-contract](ingest-contract.md) · [retro](retro.md) · [experiments](experiments.md) · [roadmap (risks, v0.1 DoD)](roadmap.md)

---

## Done (M0 partial)

- **Spike B** — session lifecycle spec + core stub. `specs/session-lifecycle.qnt`, `src/core/session.rs`, `tests/spec/session_lifecycle.rs`.

---

## M0 — Spikes (gate for M1)

**Spike A** — parse ≥100 real Cursor/Claude/Codex transcripts. Measure schema drift + edge cases.
Done: ≥80% parse clean; drift documented (issue/PR, not a repo path).

**Spike C** — minimal Axum ingest stub in `tests/ingest_stub/`. Push 10 batched events from fake sync daemon.
Contract: [ingest-contract.md](ingest-contract.md). Done: `cargo test` green; stub rejects bad idempotency key.

**Spike D** — Cursor + Claude Code hooks → `kaizen ingest hook` stub. Capture 1 event per agent E2E.
Done: both agents emit parseable JSON to stdin; proof in fixture or integration test.

**Spike E** — prototype H2/H3/H8 on SQLite vs LadybugDB, 1k synthetic sessions.
Output: recommendation feeding [ADR-001](adr/001-storage.md) (go / dual-engine / defer). *(data, not gate)*

*Gate: A–D green → proceed to M1.*

---

## M1 — Collector Tier 1 + Store (wk 2–3)

1. **Full dep tree** — add all `[dependencies]` from `architecture.md` to `Cargo.toml`. Pin exact versions.
2. **`core::event`** — `src/core/event.rs`. `Event`, `EventKind`, `Session`, `SessionStatus`. Serde. No IO. Done: round-trip serde tests green.
3. **`core::config`** — `src/core/config.rs`. Parse `.kaizen/config.toml` + `~/.kaizen/config.toml`. `[scan]`/`[sources.*]`/`[retention]`. Done: merge-order + missing-file tests green.
4. **`store::sqlite`** — `src/store/sqlite.rs`. Schema migration, append API, single-writer tokio task, WAL mode. Done: append+query round-trip; WAL contention test green.
5. **`collect::tail::cursor`** — `src/collect/tail/cursor.rs`. `notify` watcher on `~/.cursor/projects/*/agent-transcripts/`. Rotation, partial lines, backfill. Done: fixture tests `tests/fixtures/cursor/*.txt` green.
6. **`shell::cli`** — `src/shell/cli.rs`. Clap: `sessions list`, `session show <id>`. Done: `kaizen sessions list` indexes this repo's sessions.
7. **`specs/session-lifecycle.qnt` locked** — wire `quint-connect` against `collect::tail::cursor` state machine. Done: `tests/spec/session_lifecycle.rs` green with `#[quint_run]`.
8. **ADR-001** — `docs/adr/001-storage.md`. SQLite vs LadybugDB vs DuckDB. Informed by Spike E.

---

## M2 — Multi-Agent Tier 1 + Cost (wk 3–4)

9. **`collect::tail::claude`** — `src/collect/tail/claude.rs`. JSONL parser. Token/cost extraction. Fixtures: `tests/fixtures/claude/*.jsonl`.
10. **`collect::tail::codex`** — `src/collect/tail/codex.rs`. JSONL parser. Fixtures: `tests/fixtures/codex/*.jsonl`.
11. **`core::cost`** — `src/core/cost.rs`. Price table from bundled `cost.toml`. Cursor cost estimation (no native tokens → model+turns heuristic). Done: known-session cost assertion.
12. **`kaizen summary`** — aggregates sessions+events across all 3 agents. Done: shows count, total USD, by-agent, by-model.

---

## M3 — TUI + Tier 2 Hooks (wk 4–5)

13. **`ui::tui`** — `src/ui/tui.rs`. Ratatui: list view, detail view, live-tail via broadcast channel. Done: open TUI, see live session activity.
14. **`collect::hooks::cursor`** — `src/collect/hooks/cursor.rs`. Stdin JSON → `Event`. `PreToolUse`/`PostToolUse`/`Stop`/`SessionStart`. Done: Spike D fixture tests green.
15. **`collect::hooks::claude`** — `src/collect/hooks/claude.rs`. Same for Claude Code `hook_event_name` format.
16. **`kaizen ingest hook`** — reads stdin, dispatches by `--source cursor|claude`, appends to SQLite. Done: event lands in TUI within 1s.
17. **`kaizen init`** — idempotent setup. Writes `.kaizen/config.toml`, patches hooks, backs up, installs retro skill. Done: idempotent on re-run; malformed file → non-zero exit.
18. **`specs/init-setup.qnt`** — models `kaizen init` install / patch invariants. Done: rerun is noop; backups only on first hook patch.
19. **Status state machine wired** — `running`/`waiting`/`idle`/`done` from hook events. Done: `session-lifecycle.qnt` test green on live hook-driven state.

---

## M4 — Sync Daemon + Ingest Contract (wk 5–6)

20. **`redact`** — `src/redact/mod.rs`. Secrets, env vars, abs paths, git emails. Aho-corasick + regex. Done: no raw secret pattern in output; fixture before/after.
21. **`specs/redaction-completeness.qnt`** — every outbound event passes `redact`; no `/Users` in path. Done: `tests/spec/redaction_completeness.rs` green.
22. **`sync`** — `src/sync/mod.rs`. Reads `sync_outbox`. Batch (500 events / 1 MB / 10 s). UUIDv7 idempotency key. Retry + backoff. HTTPS POST. Dedup on `(team_id, workspace_hash, session_id_hash, event_seq)`. Done: retry 429; dedup 409.
23. **`specs/sync-backpressure.qnt`** — bounded outbox, retry/backoff, no dup POSTs. Done: `tests/spec/sync_backpressure.rs` green.
24. **Ingest contract finalized** — [ingest-contract.md](ingest-contract.md) + `specs/openapi/ingest-v1.yaml`.
25. **Integration test** — `tests/ingest_stub/` real Axum server. Events sync E2E. Asserts dedup + no raw secrets in payload.
26. **`kaizen sync status`** — outbox depth, last-success timestamp, error rate. Done: watch events sync to local stub.

---

## M5 — Retro v0 CLI + Skill (wk 6–7)

26. **`retro::heuristics`** — `src/retro/heuristics/`. One file per H1–H8. Pure `fn(Inputs) -> Vec<Bet>`. Ref: [retro.md §Heuristics](retro.md#heuristics-v0). Done: synthetic SQLite fixture → expected bets.
27. **`retro::engine`** — `src/retro/engine.rs`. `run(Inputs) -> Report`. Runs H1–H8, merges, ranks by `tokens_saved_per_week / (effort_minutes + 1)`, deduplicates vs prior, top-5. Pure fn.
28. **`report::md` + `report::json`** — `src/report/`. Atomic write (tmp+rename). Markdown per [retro.md §Output Format](retro.md#output-format). JSON for skill use.
29. **`retro::scheduler`** — `src/retro/scheduler.rs`. Cron (default Sun 09:00 local). File lock, single-flight per workspace.
30. **`kaizen retro`** — `--days N`, `--dry-run`, `--json`. Done: drops `.kaizen/reports/2026-WXX.md`.
31. **Skill packaging** — `kaizen init` installs `.cursor/skills/kaizen-retro/SKILL.md`. Skill invokes `kaizen retro --json --days 7`, summarizes top-3 bets.
32. **`specs/retro-pipeline.qnt`** — at-most-one retro per workspace; atomic write. Done: `tests/spec/retro_pipeline.rs` green.
33. **Backtest** — run on 4 wks own sessions. Manual bet rating. Thresholds → `docs/retro-tuning.md`. Target ≥60% actionable.

---

## M6 — Experiments v0 (wk 7–8)

34. **`experiment`** — `src/experiment/mod.rs`. `Experiment`, `Metric`, `Binding`, `Criterion`, `State`. Persist to `experiments` table. Ref: [experiments](experiments.md#data-model).
35. **`experiment::binding`** — `src/experiment/binding.rs`. git2 walk: classify session Control|Treatment|Excluded. Manual tag override. Done: fixture git repo tests green.
36. **`experiment::stats`** — `src/experiment/stats.rs`. Bootstrap CI (10k resamples, 95% percentile interval on median delta). Winsorize p1/p99. Warn N<30. Done: synthetic distributions with known effect sizes.
37. **`specs/experiment-lifecycle.qnt`** — `Draft→Running→Concluded→Archived`; binding total; no reclassify after Concluded. Done: `tests/spec/experiment_lifecycle.rs` green.
38. **`kaizen exp`** — `new`, `list`, `status`, `tag`, `report`, `conclude`. `report` produces markdown with effect size + CI. Done: new → accumulate → report with bootstrap CI.
39. **Acceptance test** — dogfood 1 real experiment on this repo. Conclude. Report verified correct.

---

## M7 — Hardening + AGPLv3 + Docs (wk 8–9)

40. **AGPLv3** — `LICENSE`. SPDX header on all `.rs`: `// SPDX-License-Identifier: AGPL-3.0-or-later`.
41. **`specs/retention.qnt`** — hot/warm/cold transitions; no hot event older than TTL. Done: `tests/spec/retention.rs` green.
42. **ADRs** — `docs/adr/`: 001-storage (M1), 002-client-only-scope, 003-no-llm-retro, 004-agplv3, 005-hybrid-experiment-binding.
43. **Docs** — `docs/structure.md`, `docs/architecture.md`, `docs/datamodel.md`, `docs/config.md`, `docs/patterns.md` all current.
44. **`cargo audit` + `cargo deny`** — `.cargo/deny.toml`. License allow-list (MIT/Apache/AGPL). No CVEs. Green in CI.
45. **CI** — `.github/workflows/ci.yml`. Jobs: `test`, `clippy` (-D warnings), `fmt`, `quint`, `audit`. All on push+PR.
46. **Release script** — `scripts/release.sh`. Bump version, tag, cross-compile macOS+Linux via `cross`, publish to crates.io.
47. **README quickstart** — `cargo install kaizen && kaizen init`. Shows `sessions list`, `summary`, `retro`, `tui`. CI badge.
48. **Pilot** — 2 partner teams, 4-wk trial. Verify: no data-loss, ≥1 retro bet applied, ≥1 experiment concluded per team.
