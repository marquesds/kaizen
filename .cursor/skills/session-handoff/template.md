# Handoff: [TODO: short title]

**Created:** [TODO: YYYY-MM-DD HH:MM:SS]
**Agent:** [TODO: cursor | claude-code | codex]
**Branch:** [TODO: git branch]
**Continues from:** [TODO: .handoffs/<prior>.md OR "none"]

---

## Current State

[TODO: 2-4 sentences. What's happening right now. What was last action.
What's immediate blocker or next move.]

## Critical Context

What next agent MUST know to not break things:

- [TODO: assumption #1 — e.g. "Storage trait not yet implemented, only in-memory stub"]
- [TODO: assumption #2 — e.g. "Clippy errors suppressed in src/collector.rs line 42, fix next"]
- [TODO: gotcha — e.g. "src/main.rs line 17 has hardcoded port, will use env var next"]

## Next Steps

Ordered. First item must be runnable as written.

1. [TODO: concrete action — e.g. "Run `cargo test tests/collector` to confirm 3 failing tests"]
2. [TODO: next action]
3. [TODO: next action]

## Decisions Made

| Decision | Chose | Rejected | Why |
|---|---|---|---|
| [TODO: e.g. storage backend] | [TODO: in-memory] | [TODO: sqlite] | [TODO: simpler, no deps for MVP] |

## Modified Files

| File | Purpose |
|---|---|
| [TODO: src/collector.rs] | [TODO: added ingest fn, line 38] |
| [TODO: tests/...] | [TODO: new test for ingest] |

## Verification Status

- `cargo test`: [TODO: pass | fail (N tests) | not run]
- `cargo clippy -- -D warnings`: [TODO: pass | fail | not run]
- Manual checks: [TODO: e.g. "binary starts and accepts connections"]

## Open Questions

- [TODO: question #1, or "none"]

## Related

- Spec: [TODO: specs/...md OR "none"]
- Docs: [TODO: docs/<file>.md reference OR "none"]
- Issue: [TODO: link OR "none"]
