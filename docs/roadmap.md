# Product roadmap (v0.1)

High-level scope, risks, and “done” criteria. **Execution order and
file-level tasks:** [impl-sequence.md](impl-sequence.md).

## Milestone status (summary)

| Milestone | Theme | Status |
|---|---|---|
| M0 | Spikes (risk reduction) | In progress |
| M1 | Collector tier 1 + SQLite store | Planned |
| M2 | Multi-agent + cost | Planned |
| M3 | TUI + tier 2 hooks | Planned |
| M4 | Sync daemon + ingest | Planned |
| M5 | Retro CLI + skill | Planned |
| M6 | Experiments v0 | Planned |
| M7 | Hardening, license, docs, CI | Planned |

## Risks (top 6)

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Cursor / Claude transcript or hook format changes | High | High | Pin parser version, fixture suite, fail-soft on unknown lines/keys. |
| Hook payloads don't carry expected context | Med | Med | Spike D in M0 confirms before M3. Fall back to tail-only if hooks too thin. |
| Anonymization removes too much signal for retro | Med | High | Backtest M5 with redacted vs un-redacted. Tunable redaction levels. |
| SQLite contention with concurrent retro/sync readers | Low | Med | WAL mode, single writer task, load test in M1. |
| Quint authoring overhead slows team | Med | Med | Keep specs small (one file per state machine), only spec invariants that matter. |
| Heuristic retro produces noise | High | Med | Mandatory backtest in M5 before ship, tunable thresholds, dedup vs prior reports. |
| Scope creep (LLM retro, web UI, T3 proxy, server) | High | High | v0.1 pack is the contract. Anything new = new planning cycle, not a silent stretch. |

## Assumptions to re-validate (M3)

- Tier 2 hooks deliver enough new signal to justify install friction.
- `kaizen init` patching of `.cursor/hooks.json` + `.claude/settings.json` is
  non-destructive and idempotent.
- Heuristic bets land as actionable (≥ 50% rated useful).

## Out of scope (v0.1)

Requires a new discovery/planning cycle, not a silent stretch:

- Web UI, server backend, multi-tenant, RBAC.
- LLM-driven bets, auto-PR, diff-risk score, replay UI.
- Skill marketplace, policy-as-code, A/B framework beyond simple binding.
- Gemini / Pi / OpenCode parsers.
- Tier 3 HTTP proxy collection.

## Definition of done (v0.1)

- `cargo install kaizen` works on macOS + Linux.
- `kaizen init` in any repo writes `.kaizen/config.toml`, patches
  `.cursor/hooks.json` + `.claude/settings.json`, installs
  `.cursor/skills/kaizen-retro/SKILL.md`.
- Three agents (Cursor / Claude Code / Codex) ingest via tier 1 + tier 2
  with no extra config.
- TUI (`kaizen tui`) shows live session activity.
- Sync daemon ships anonymized events to a configured ingest endpoint;
  `kaizen sync status` reports health; integration test against stub
  passes in CI.
- `kaizen retro` produces a markdown report with ≥ 3 bets; agent skill
  triggers in Cursor + Claude Code on expected phrases.
- One full experiment run end-to-end (`exp new` → events across binding
  boundary → `exp report` with bootstrap CI).
- All Quint specs pass via `quint-connect` in CI.
- Two partner teams running it for 2+ weeks, no data-loss incidents,
  ≥ 1 retro bet applied and ≥ 1 experiment concluded per team (pilot).

## Related docs

- [ingest-contract.md](ingest-contract.md) — HTTP contract for sync → server.
- [retro.md](retro.md) — heuristic retro engine (M5).
- [experiments.md](experiments.md) — experiment lifecycle (M6).
- [architecture.md](architecture.md) — module boundaries (fill as code grows).
