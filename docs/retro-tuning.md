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
| H9 | `MIN_ERROR_EVENTS`, `MIN_SESSIONS`, `SESSION_ERROR_SHARE` | ≥ 6 errors **or** ≥ 22% of ≥ 5 sessions with an error |
| H10 | `MIN_FAILING_RESULTS` | ≥ 3 failing shell-like tool results in one session |
| H11 | `MIN_SESSIONS`, `OUTLIER_VS_MEAN`, `MIN_MAX_COST_E6` | ≥ 6 sessions; max session ≥ 4× mean cost and ≥ 40k µUSD |
| H12 | `MIN_LOC`, `MIN_BYTES`, `MIN_READS_ON_PATH` | Read-like tool on path with LOC/bytes thresholds, ≥ 2 reads |
| H13 | `MIN_TOOL_CALLS`, `MIN_MCP_SHARE`, `MIN_SESSIONS_TOTAL`, `MIN_SUBAGENT_SESSION_SHARE` | MCP share or subagent session share thresholds |
| H14 | `MIN_COMBINED_ITEMS`, `MIN_TOTAL_BYTES`, `MIN_ITEMS_WITH_BYTES` | Many on-disk rules/skills or very large combined bytes |
| H33 | `MIN_RUN_LEN`, `MIN_SESSIONS_WITH_RUN`, `DOUBLE_RUN_LEN`, `TOKENS_PER_EXTRA_CALL` (runs); `MIN_SUBSEQ_REPEATS_LEN2`, `MIN_SUBSEQ_REPEATS_LEN3`, `TOKENS_PER_CYCLE`, `MULTI_SESSION_MULT` (subseq) | Long same-tool streaks vs cross-session / double-length gate; repeating 2-grams / 3-grams and token multiplier when multiple sessions show the pattern |

## Redaction vs signal

Compare `kaizen retro` on the same window with sync redaction enabled and disabled only in a **safe** copy of the database if you need to validate that path redaction does not starve H2/H8. Do not commit raw secrets.

## Notes (fill in after backtests)

- Date / window:
- % actionable:
- Changes made:
