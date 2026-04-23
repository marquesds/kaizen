# Retro tuning (M5)

Heuristic thresholds live in `src/retro/heuristics/h*.rs`. Change constants there and re-run tests.

## Backtest procedure

1. Ensure local sessions are ingested: `kaizen summary` in this repo.
2. Run `kaizen retro --days 28 --dry-run` (or `--json`) and save the output.
3. For each bet in `top_bets`, label **actionable**, **noise**, or **wrong**.
4. Target: ≥ 60% actionable on your own data before widening rollout.

## Threshold reference

| Heuristic | Constant | Default meaning |
|-----------|----------|-----------------|
| H1 | `MIN_EVENTS_FOR_CREDIBLE`, `STALE_EDIT_MS` | Need enough events; skill dir mtime older than 60d |
| H2 | `MIN_PAIR_SESSIONS` | Co-edit pair appears in ≥ 3 sessions, different top-level path |
| H3 | `MIN_TOUCHES` | Same path touched ≥ 4× in one session |
| H4 | `TOP_SHARE`, `MIN_EVENTS` | Tool is ≥ 25% of tool calls, ≥ 15 tool events total |
| H5 | `IDLE_MS`, implicit count | ≥ 2 sessions idle ≥ 30 minutes |
| H6 | payload heuristics | ≥ 10 skill-like payloads, ≥ 70% “ignored” pattern |
| H7 | `MIN_SESSIONS`, `MAX_AVG_COST_E6`, premium share | Many sessions, low average cost, premium-heavy |
| H8 | drift hit count | ≥ 3 doc-read → code-edit alternations per session |

## Redaction vs signal

Compare `kaizen retro` on the same window with sync redaction enabled and disabled only in a **safe** copy of the database if you need to validate that path redaction does not starve H2/H8. Do not commit raw secrets.

## Notes (fill in after backtests)

- Date / window:
- % actionable:
- Changes made:
