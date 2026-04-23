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
| H1 | Dead Skill | Skill file in `.cursor/skills/` never in `skills_used` 30d AND not edited 60d | "Delete `.cursor/skills/<name>/`. Saves description-frontmatter tokens/session." | `description_bytes / 4 × sessions/day × 30` |
| H2 | Hot File Cluster | Pair of files co-edited ≥ 5× in 14d, different modules | "Refactor `<a>` + `<b>` — hidden coupling, agents pay context cost loading both." | `co_edit_count × avg_context_tokens` |
| H3 | Repeated Failed Edit | Same file ≥ 3× edit→test_fail→edit cycles in single session, recurring | "Add guardrail/test/invariant for `<file>` — agents loop here." | `loop_count × avg_loop_tokens` |
| H4 | High-Cost Tool | Tool call (e.g. full-file read >500 lines) in top-10 cost contributors | "Tighten `read-hygiene` rule. $X/week from `<tool>`." | direct cost sum |
| H5 | Idle Session Bloat | Sessions >30 min `idle` with no `done`, avg rising WoW | "Tune `scan.idleTtlMinutes`. Inflates active counts + sync noise." | count × bytes |
| H6 | Skill Trigger Misfire | Skill triggered but `outcome = ignored` >70% | "Rewrite `<skill>` description. Wastes context budget." | `trigger_count × description_bytes` |
| H7 | Model Mismatch | Cheap model loops/fails; expensive model on trivial reads | "Flip model routing for `<task pattern>`." | `cost_delta × frequency` |
| H8 | Doc Drift | Agent reads `docs/<x>.md` then edits contradicting source ≥ 3× in 14d | "Update `docs/<x>.md`. Agents waste turns reconciling." | `contradict_count × recovery_tokens` |

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
