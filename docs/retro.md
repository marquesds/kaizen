# Retro — Heuristic Engine v0

Weekly job. Reads trailing N days of sessions + repo state. Produces
ranked markdown report of bets to make agents cheaper, faster, more
accurate in **this** codebase.

No LLM in v0. Deterministic, specifiable, cheap, replayable.

**Engine is a pure library.** Same `retro::engine::run(Inputs) -> Report`
called from CLI, scheduler, and the `kaizen-retro` agent skill (below).
No side effects in the engine — IO at boundary only.

## Goal

3+ actionable bets per week, each with:

- Hypothesis (what changes, why).
- Expected impact (tokens saved/week, success-rate uplift, time saved).
- Cost (effort to apply).
- Evidence (links to sessions/files).
- Apply step (manual command or PR template).

## Inputs

| Input | Source |
|---|---|
| Session events (last N days) | SQLite `events`, `sessions` |
| Files touched | SQLite `files_touched` |
| Skills triggered + outcome | SQLite `skills_used` |
| Repo state | `git ls-files`, file sizes, mtimes |
| Skill/rule files | `.cursor/skills/`, `.cursor/rules/`, `AGENTS.md` |
| Past reports | `.kaizen/reports/*.md` (avoid repeat bets) |

## Heuristics (v0)

Each heuristic = pure function `Inputs → Vec<Bet>`. Easy to test, easy to spec.

| ID | Name | Signal | Bet | Impact estimate |
|---|---|---|---|---|
| H1 | Dead Skill / Rule | On-disk skill or `.mdc` rule unused in 30d lookback and not edited recently (stale mtime) | Remove or merge unused `.cursor/skills/<slug>/` or `.cursor/rules/*.mdc`. | size × usage proxy |
| H2 | Hot File Cluster | Pair of files co-edited in ≥ 3 distinct sessions, different top-level path (window = `--days`) | Refactor `<a>` + `<b>` — hidden coupling. | co-edit count × complexity |
| H3 | Path churn | Same path touched by ≥ 4 tool calls in one session (edit-loop proxy, not literal test failures) | Tighten guardrails for `<path>`. | touches × complexity |
| H4 | Dominant tool | One tool ≥ 25% of aggregated tool events and ≥ 15 tool events total | Review read/search shortcuts for that tool. | calls + cost/reasoning proxy |
| H5 | Idle bloat | ≥ 2 sessions stay `Idle` ≥ 30 minutes | Tune idle TTL / end sessions explicitly. | idle count × constant |
| H6 | Skill misfire | ≥ 10 skill-like payloads, ≥ 70% “ignored” pattern | Rewrite skill description / triggers. | ignored × constant |
| H7 | Premium overkill | ≥ 8 sessions, low average $/session, many “premium” model names | Route mechanical work to smaller models. | sessions × constant |
| H8 | Doc drift | Same session: read under `docs/` then ≥ 3 edits to `.rs`/`.ts`/`.md` elsewhere | Refresh docs vs implementation. | drift hits × complexity |
| H9 | Error budget | ≥ 6 `Error` events **or** ≥ 22% of sessions (≥ 5 sessions) with ≥ 1 error | Fix flaky tools / proxy / permissions. | error count × constant |
| H10 | Shell / test failures | ≥ 3 failing shell-like `ToolResult`s in one session (`is_error` or exit/test-fail text heuristics) | Stabilize command or CI signal. | failures × constant |
| H11 | Cost outlier | ≥ 6 sessions; one session’s attributed cost ≥ 4× the per-session mean and ≥ $0.04 | Inspect longest / hottest session. | cost delta proxy |
| H12 | Large file reads | Read-like tool hits a path with `file_facts` LOC ≥ 500 or bytes ≥ 80k, ≥ 2 reads | Read-hygiene / split file. | reads × LOC |
| H13 | Delegation load | MCP: ≥ 12% of tool calls are MCP-named (≥ 20 calls). Subagents: ≥ 15% of sessions have `trace_path` under `subagents/` (≥ 6 sessions; local Cursor best) | Reduce MCP chatter or subagent fan-out. | calls or sessions × constant |
| H14 | Instruction bloat | ≥ 22 skill + rule files on disk **or** ≥ 140 KiB combined (≥ 10 items) | Consolidate rules/skills. | bytes / 8 proxy |

**Provider-only note:** Remote cache rows omit `tool_spans` / `files_touched` / local indexes; H12 still uses local `file_facts` when indexed. H13 subagent detection relies on `trace_path` (often empty on synthetic remote sessions).

## Ranking

Score = `expected_tokens_saved_per_week / (apply_effort_minutes + 1)`.
Top 5 by score → report. Ties broken by recency of evidence.

## Output Format

```
# Kaizen Retro — Week 2026-W17

Span: 2026-04-15 → 2026-04-22 · Sessions: 47 · Cost: $42.10

## Top Bets

### 1. Delete unused skill `cursor-guide` (H1)
- Saves ~1.2k tokens/session × 47 sessions = ~56k tokens/week (~$0.84)
- Evidence: 0 invocations in 30d, last edit 78d ago
- Apply: rm -rf .cursor/skills/cursor-guide
- Effort: 2 min

### 2. ...

## Skipped Bets (deduped vs prior reports)
- ...

## Raw Stats
| Metric | Value |
|---|---|
| Sessions | 47 |
| Total cost | $42.10 |
| Top model | claude-4.6-sonnet (62%) |
| Top tool | read (38% of calls) |
| Median session | 14 min |
```

## CLI

```bash
kaizen retro                       # run for last 7 days, write report
kaizen retro --days 30             # custom window
kaizen retro --dry-run             # print, do not write
kaizen retro --json                # emit Report as JSON to stdout (skill uses this)
kaizen retro --apply <bet-id>      # interactive apply (v0.2)
```

## Retro as Agent Skill

Ships with the binary. Installed at `.cursor/skills/kaizen-retro/SKILL.md`
in the consuming repo (or auto-discovered if user runs `kaizen init`).

Skill frontmatter:

```yaml
---
name: kaizen-retro
description: >
  Surface ranked bets to make agents cheaper, faster, more accurate in
  this codebase. Use when user asks "what should I improve", "run retro",
  "agent productivity bets", "audit my skills", or invokes /retro.
---
```

Skill body invokes `kaizen retro --json --days 7`, parses the `Report`,
summarizes top 3 bets to user with: hypothesis, expected impact, evidence
links, apply step. Suggests one as next action. No autonomous apply.

Why both surfaces:

- **CLI / scheduler** — recurring discipline, weekly markdown for the team.
- **Skill** — on-demand, in-flow, when dev asks "what's wrong with my
  agent setup right now?". Cheap because engine is pure.

## Quint Spec

Spec retro pipeline as state machine:

```
states: Idle → Loading → Computing → Ranking → Writing → Idle
invariants:
  - At most one retro running per workspace at a time
  - Report file written atomically (tmp + rename)
  - Bet ranking is total order on (score, recency, id)
  - Deduped bets vs last report never appear in current
```

## Validation Plan

Before shipping:

1. Backtest on this repo, 4 weeks of sessions. Manually rate each
   bet: actionable / noise / wrong. Target ≥ 60% actionable.
2. Run on 2 partner team repos. Same rating. Target ≥ 50%.
3. Iterate heuristic thresholds based on noise rate.

## Future (v0.2+)

- LLM augmentation: take top heuristic bets, ask LLM to refine
  hypothesis + draft PR. Spec the prompt boundary in Quint.
- Auto-apply low-risk bets (delete unused skill, tighten rule).
- A/B framework: split team between "applied bet" and "control",
  measure success-rate delta over 2 weeks.
- Learn from feedback: thumbs-up/down on bets feeds heuristic weights.
