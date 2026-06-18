# Retro — Heuristic Engine v0

`kaizen retro` reads recent sessions and cached local repo facts, then produces
a ranked Markdown report of changes that may make agents cheaper, faster, or
more accurate in this codebase. Default runs are cache-first. Use `--refresh`
when the local store may be stale; it rescans agent transcripts first and can
take a while on large workspaces.

The engine is deterministic and pure:
`retro::engine::run(Inputs) -> Report`. CLI commands, schedulers, and the
`kaizen-retro` agent skill call the same library. IO happens at the boundary.

## Goal

A useful weekly report should contain at least three actionable bets. Each bet
includes a hypothesis, evidence, expected token savings, confidence, action
category, effort estimate, and apply step.

## Inputs

| Input | Source |
|---|---|
| Session events | SQLite `events`, `sessions` |
| Files touched | SQLite `files_touched` |
| Skills triggered | SQLite `skills_used` |
| Repo state | `git ls-files`, file sizes, mtimes |
| Skill/rule files | `.cursor/skills/`, `.cursor/rules/`, `AGENTS.md` |
| Past reports | `~/.kaizen/projects/<slug>/reports/*.md` |
| Optional outcomes | test/lint rows, process samples, feedback, eval scores |

## Heuristics

Each heuristic is a pure function from `Inputs` to `Vec<Bet>`. The engine adds
confidence and category after heuristics run.

| ID | Name | Signal | Bet | Confidence | Category |
|---|---|---|---|---|---|
| H1 | Dead Skill / Rule | On-disk skill or `.mdc` rule unused in 30d lookback and not edited recently | Propose reviewed removal through `kaizen guidance` | High | QuickWin |
| H2 | Hot File Cluster | Pair of files co-edited in at least 3 sessions across top-level paths | Refactor hidden coupling | Medium | Investigation |
| H3 | Path churn | Same path touched by at least 4 tool calls in one session | Tighten guardrails for path | Medium | Investigation |
| H4 | Dominant tool | One tool is at least 25% of events and has at least 15 calls | Review read/search shortcuts | Low | Hygiene |
| H5 | Idle bloat | At least 2 sessions idle for 30+ minutes | Tune idle TTL or end sessions | Low | Hygiene |
| H6 | Skill misfire | Skill-like payloads are often ignored | Rewrite skill description/triggers | Low | Hygiene |
| H7 | Premium overkill | Many short or cheap sessions use premium model names | Route mechanical work to smaller models | Medium | Hygiene |
| H8 | Doc drift | Session reads docs, then edits implementation elsewhere | Refresh docs versus implementation | Low | Hygiene |
| H9 | Error budget | Many error events or sessions with errors | Fix flaky tools, proxy, or permissions | High | Investigation |
| H10 | Shell / test failures | Repeated failing shell-like tool results | Stabilize command or CI signal | High | Investigation |
| H11 | Cost outlier | One session costs at least 4x mean and at least $0.04 | Inspect longest or hottest session | Medium | Investigation |
| H12 | Large file reads | Repeated reads of large indexed files | Split file or improve read hygiene | High | Investigation |
| H13 | Delegation load | High MCP call share or frequent subagent sessions | Reduce chatter or fan-out | Low | Hygiene |
| H14 | Instruction bloat | Many skill/rule files or large combined bytes | Consolidate rules/skills | Medium | Investigation |
| H15 | Low eval scores | Multiple low LLM-as-judge scores or score trend drop | Review low-scoring sessions | Low | Hygiene |
| H16 | Prompt variant association | Fingerprint group is associated with worse cost or error rate | Diff variants, then validate with prompt binding | Low | Hygiene |
| H17 | Human feedback struggles | Bad/regression feedback or low mean score | Review flagged sessions | Low | Hygiene |
| H18 | Deep span tree | Tool span depth or fan-out crosses threshold | Flatten tool chain | Low | Hygiene |
| H19 | Context pressure | Sessions use at least 80% of context window | Split sessions or prune instructions | High | Investigation |
| H20 | Cold cache | Anthropic proxy cache read ratio is low | Stabilize prompt-cache prefix | Medium | Hygiene |
| H21 | Rate-limit cascade | Many retries in one or more sessions | Route or batch requests | High | Investigation |
| H22 | Truncation rate | At least 10% of proxy turns stop at max tokens | Raise budget or decompose tasks | Medium | Investigation |
| H23 | Todo abandonment | Large todo snapshots finish less than 40% | Narrow scope | Low | Hygiene |
| H24 | Reject rate | Hooked edit rejects cross threshold | Tighten rules and smaller diffs | Medium | Investigation |
| H25 | Mode thrash | Many mode transitions per session | Stabilize planning up front | Low | Hygiene |
| H27 | Outcome test failures | Test failure rate above 20% with at least 5 runs | Stabilize tests first | High | Investigation |
| H28 | Revert churn | Reverted lines over 14d cross threshold | Use smaller steps and rebase sooner | Medium | Hygiene |
| H29 | Lint / test debt | Clippy errors or failed tests in outcomes | Fix top lint/test debt | High | QuickWin |
| H30 | High agent CPU | Process samples exceed CPU threshold | Smaller tasks or runaway-tool check | Medium | Hygiene |
| H31 | High agent RSS | Peak sampled RSS is about 1 GiB or higher | Trim context or restart sessions | Medium | Hygiene |
| H32 | Long sampled session | A session has at least 100 process samples | Break work into shorter sessions | Medium | Hygiene |
| H33 | Automation cue | Consecutive or repeated tool-call patterns | Batch with helper script or skill | High | Investigation |

`confidence: low` means the signal is a proxy or best-effort local inference.
Low-confidence bets can still be useful, but they should read as hygiene or
follow-up work, not as proven root cause.

Provider-only note: remote cache rows may omit `tool_spans`, `files_touched`,
or local indexes. H12 still uses local `file_facts` when indexed. H13 detects
subagents through `parent_session_id` when present, then falls back to
`trace_path` containing `subagents/`.

## Ranking

Score:

```text
confidence_weight × expected_tokens_saved_per_week / (effort_minutes + 1)
```

Weights:

| Confidence | Weight |
|---|---:|
| High | 1.0 |
| Medium | 0.6 |
| Low | 0.3 |

The engine sorts by score, evidence recency, then bet id. It dedupes previous
reports and selects the report body as `1 + 2 + 2`:

| Section | Rule |
|---|---|
| High-Confidence Bet | One highest-scoring high-confidence bet |
| To Investigate | Up to two medium/high investigation bets |
| Quick Hygiene | Up to two quick-win or hygiene bets |

`Report.top_bets` remains a single `Vec<Bet>` for JSON compatibility. The order
is high-confidence bet first, then investigations, then quick hygiene.

## Output Format

```markdown
# Kaizen Retro — Week 2026-W17

Span: 2026-04-15 → 2026-04-22 · Sessions: 47 · Cost: $42.10

## High-Confidence Bet

### 1. Stabilize failing shell commands (H10 · High · investigation)
- Hypothesis: Three sessions hit repeated failing shell results before recovery.
- Evidence: 3 failing command clusters · sessions s_42, s_45, s_51
- Saves ~1200 tokens/week (est.) · Confidence: High
- Effort: 45 min · Apply: Add a repo-local smoke command.

## To Investigate

### 2. Split `src/store/projector.rs` (H2 · Medium · investigation)
- Hypothesis: This file co-edits with shell/reporting code in four sessions.
- Evidence: Co-edit count: 4 · combined complexity: 88
- Saves ~7280 tokens/week (est.) · Confidence: Medium
- Effort: 120 min · Apply: Extract shared projection logic.

### 3. Reduce large file reads in `src/main.rs` (H12 · High · investigation)
- Hypothesis: Agents repeatedly read a large command file before small edits.
- Evidence: 5 read-like calls · 934 LOC
- Saves ~9340 tokens/week (est.) · Confidence: High
- Effort: 40 min · Apply: Move command handlers into focused modules.

## Quick Hygiene

### 4. Delete unused skill `cursor-guide` (H1 · High · quick_win)
- Hypothesis: The skill is on disk but has not fired in the lookback window.
- Evidence: 0 invocations in 30 days · last edit 78 days ago
- Saves ~56000 tokens/week (est.) · Confidence: High
- Effort: 5 min · Apply: review `kaizen guidance propose --artifact skill:cursor-guide`

## Raw Stats

| Metric | Value |
|---|---|
| Sessions | 47 |
| Total cost | $42.10 |
| Top model | claude-sonnet (62%) |
| Top tool | read_file (38%) |
| Median session | 14 min |
```

If no high-confidence bet exists, the renderer omits the section and prints a
warning that remaining bets are exploratory.

## CLI

```bash
kaizen retro                       # run for last 7 days, write report
kaizen retro --days 30             # custom window
kaizen retro --dry-run             # print, do not write
kaizen retro --json                # emit Report as JSON to stdout
```

## Retro as Agent Skill

Kaizen can install a `kaizen-retro` skill into consuming repos. The skill calls
`kaizen retro --json --days 7`, parses the `Report`, and summarizes the best
bets with hypothesis, impact, evidence, confidence, and apply step. It suggests
one next action but does not apply changes.

## Quint Spec

`specs/retro-pipeline.qnt` models pipeline phase and locking behavior:

```text
Idle → Loading → Computing → Ranking → Writing → Idle
```

Core invariants:

- At most one retro runs per workspace at a time.
- Report files are written atomically with temp-file rename.

Ranking, metadata assignment, dedupe, and `1 + 2 + 2` selection are pure data
transforms covered by Rust unit tests in `src/retro/engine.rs`. Add a new Quint
module only if selection becomes stateful, concurrent, or coupled to IO.

## Validation Plan

Before shipping heuristic changes:

1. Backtest on this repo over four weeks of sessions.
2. Manually rate each bet as actionable, noisy, or wrong.
3. Run on two partner repos and compare noise rate.
4. Tune thresholds until at least 60% of local bets and 50% of partner bets
   are actionable.
