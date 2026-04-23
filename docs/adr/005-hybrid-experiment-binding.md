# ADR 005: Hybrid Experiment Binding

## Status
Accepted

## Context
Experiments (M6) need to classify each session as control / treatment /
excluded. Options: pure manual tagging, pure git ancestry, hybrid.

## Decision
Hybrid. Manual tag always wins; git ancestry is the default fallback.

Resolution order, per session:
1. `experiment_tags(experiment_id, session_id)` row → that variant.
2. Else if session's `start_commit` is ancestor of `treatment_commit` only → Treatment.
3. Else if ancestor of `control_commit` only → Control.
4. Else → Excluded (straddled boundary, no commit, or detached).

## Rationale
- Git binding auto-classifies without user work for the 90% case (lands
  a change, wait 14d, report).
- Manual override covers edge cases: sessions straddling the boundary,
  experiments over non-git artifacts (skill files, config), and
  exploratory A/B where the agent itself picks variant.
- `Branch` variant is a special case of `GitCommit` (tips resolved at
  eval time) — same code path.

## Alternatives
- Manual only: rejected — tedious for long-running experiments.
- Git only: rejected — breaks for non-commit changes and straddle-edits.
- Config flag at session start: rejected — agent doesn't know which
  experiments are live.

## Consequences
- `experiment_tags` table is part of the schema (small, keyed on
  `(experiment_id, session_id)`).
- Concluded experiments MUST freeze their manual-tag set; reclassification
  after conclusion is a spec invariant.
- Pure session binding in `src/experiment/binding.rs`; git shell calls
  pushed to one function (`is_ancestor`).
