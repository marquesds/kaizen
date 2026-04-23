# ADR 002: Cost Estimation Strategy

## Status
Accepted

## Context
Need cost data per session/event. Three agents, different fidelity:
- Claude Code: native `tokens.inputTokens/outputTokens` in transcript
- Codex: native `usage.prompt_tokens/completion_tokens`
- Cursor: no native token data emitted

Cost unit: `cost_usd_e6` — integer micro-dollars (10^-6 USD). No float.
Reason: avoids rounding drift across millions of events; integer addition is associative.

## Decision
- Claude/Codex: extract tokens at parse time, store in `events.tokens_in/out`
- Cost computed from `assets/cost.toml` price table via `core::cost::CostTable::estimate`
- Cursor heuristic: model=None → `cursor` entry in cost.toml, avg 5k tokens/turn estimate
- Re-evaluate at M4 when Tier 3 proxy provides real upstream token counts

## Alternatives
- f64 per event: float rounding accumulates; rejected
- Server-side cost API: adds network dep; rejected for local-first tool
- Per-session flat fee: too coarse; rejected

## Consequences
- Cursor costs are estimates, not actuals — labeled in output
- Price table bundled at compile time; update requires rebuild (acceptable for v0.1)
- M4 proxy can backfill real token counts; existing estimates stay labeled "estimated"
