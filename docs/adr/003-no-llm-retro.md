# ADR 003: No-LLM Retro Engine

## Status
Accepted

## Context
Retro engine ranks "bets" (improvements). Option: call LLM to
generate bets from raw session data. Option: deterministic heuristics.

## Decision
Deterministic heuristics only (H1–H8). No LLM in retro compute path.

## Rationale
- Reproducible: same inputs, same report — enables CI + backtests.
- Cheap: `kaizen retro` runs locally, no API key, no network.
- Auditable: bets trace to rule IDs + evidence rows; no hallucination risk.
- Privacy: zero data leaves workspace on `retro`. Sync path is explicit opt-in.

## Alternatives
- Hybrid: heuristic shortlist, LLM polish. Rejected — adds cost without
  improving rank quality measurably on dogfood set.
- LLM-only: rejected — non-reproducible, non-auditable, offline-hostile.

## Consequences
- Heuristic library grows over time (H9+). Bar: each new rule includes a
  test fixture and a dogfood trace.
- Skill layer (`kaizen-retro`) *does* use LLM for summarization at the
  agent, not in `kaizen` itself.
