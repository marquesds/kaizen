# Roadmap

Milestone snapshot for v0.1. Detail lives in [CHANGELOG.md](CHANGELOG.md)
as milestones ship.

| Milestone | Theme | Status |
|---|---|---|
| M0 | Spikes (risk reduction) | Done |
| M1 | Collector tier 1 + SQLite store | Done |
| M2 | Multi-agent + cost | Done |
| M3 | TUI + tier 2 hooks | Done |
| M4 | Sync daemon + ingest | Done |
| M5 | Retro CLI + skill | Done |
| M6 | Experiments v0 | Done |
| M7 | Hardening + OSS + CI | In progress |

## v0.1 definition of done

- `cargo install kaizen` works on macOS + Linux.
- `kaizen init` is idempotent.
- Cursor / Claude Code / Codex ingest via tier 1 + tier 2.
- `kaizen tui` shows live session activity.
- Sync daemon ships redacted events; `kaizen sync status` reports health.
- `kaizen retro` produces a Markdown report with ≥ 3 bets.
- One full experiment run (`exp new` → `exp report` with bootstrap CI).
- All Quint specs pass in CI.

## Out of scope (v0.1)

New discovery cycle required, not a silent stretch:

- Web UI, server backend, multi-tenant, RBAC.
- LLM-driven bets, auto-PR, diff-risk score, replay UI.
- Skill marketplace, policy-as-code, richer A/B framework.
- Gemini / Pi / OpenCode parsers.
- Tier 3 HTTP proxy collection.

## Related

- [CHANGELOG.md](CHANGELOG.md) — release notes.
- [docs/concepts.md](docs/concepts.md) — vocabulary.
- [docs/architecture.md](docs/architecture.md) — module boundaries.
