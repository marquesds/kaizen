---
name: git-workflow-and-versioning
description: >
  Git workflow and commit discipline. Use when making any code change. Covers
  trunk-based development, atomic commits, change sizing, and the
  commit-as-save-point pattern.
---

# Git Workflow and Versioning

Trunk-based development with atomic commits. Every commit leaves system working.

## Commit Discipline

### Atomic Commits

Each commit: one logical change. Builds, tests pass, independently revertable.

```bash
# GOOD: one logical change per commit
git commit -m "Add session ingestion to collector module"
git commit -m "Add HTTP handler for session ingest endpoint"
git commit -m "Add tests for session ingestion"

# BAD: everything in one commit
git commit -m "Add ingestion, fix bugs, refactor storage, update config"
```

### Commit Messages

First line: short, imperative, standalone. Body: what and why.

```
feat(collector): add session ingestion

Ingests session telemetry events from agent runtime.
Batches events every 5s before flushing to storage.
```

Conventions:
- `feat:` new feature
- `fix:` bug fix
- `refactor:` restructure without behavior change
- `test:` add or update tests
- `docs:` documentation only
- `chore:` tooling, deps, config

### Change Sizing

```
~100 lines  → Good. Reviewable in one sitting
~300 lines  → Acceptable if single logical change
~1000 lines → Too large. Split it
```

## Trunk-Based Development

- Work on short-lived feature branches
- Merge to main frequently (at least daily)
- Keep branches small and focused
- Delete branches after merge

## Commit-as-Save-Point Pattern

```
1. Implement slice → tests pass → commit
2. Implement next slice → tests pass → commit
3. Something breaks → git stash or revert to last good commit
```

## Pre-Commit Verification

```bash
cargo test && cargo clippy -- -D warnings && cargo fmt --check
```

## Branching Strategy

```
main ──────────────────────────────────→
  \                    /
   feat/add-collector ──  (short-lived, <1 day ideally)
```

## Common Rationalizations

| Rationalization | Reality |
|---|---|
| "I'll commit when done" | Commit at each working state. Lose work = lose time |
| "Commit too small" | Small commits free. Large commits hide bugs |
| "I'll squash later" | Squashing loses valuable history. Commit well from start |

## Red Flags

- Large uncommitted changes accumulating
- Commits that break build
- Vague messages ("fix stuff", "WIP", "updates")
- Long-lived feature branches (>2 days)
- Mixing refactoring and feature work in one commit
