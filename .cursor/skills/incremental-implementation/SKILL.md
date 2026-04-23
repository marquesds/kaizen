---
name: incremental-implementation
description: >
  Delivers changes in thin vertical slices. Use when implementing any feature
  touching more than one file. Use when tempted to write a large amount of code
  at once, or when a task feels too big to land in one step.
---

# Incremental Implementation

Build in thin vertical slices: implement, test, verify, commit, repeat.

## Increment Cycle

```
Implement → Test → Verify → Commit → Next slice
```

Each slice leaves system in working, testable state.

## Slicing Strategies

### Vertical Slices (Preferred)

```
Slice 1: Ingest event (struct + parse + unit test)
Slice 2: Store event (storage trait + impl + test)
Slice 3: Query events (query fn + integration test)
```

### Risk-First Slicing

Tackle riskiest piece first. Discover failure early.

## Rules

### Rule 0: Simplicity First

Ask: "What's simplest thing that could work?"

```
BAD:  Actor system with back-pressure for one notification
GOOD: Simple function call

BAD:  Trait + multiple impls for two similar modules
GOOD: Two straightforward structs
```

### Rule 0.5: Lazy Doc Reads

Read `docs/` only when that slice requires it — and only the section being edited.

```
# GOOD — reading only the relevant section
Read: path="docs/architecture.md" offset=20 limit=30

# BAD — reading full doc upfront "just in case"
Read: path="docs/architecture.md"
```

### Rule 1: Scope Discipline

Touch only what task requires. Don't "clean up" adjacent code.

```
NOTICED BUT NOT TOUCHING:
- src/helpers.rs has unused function (unrelated)
- Cargo.toml could be reorganized (separate task)
→ Want me to create tasks for these?
```

### Rule 2: One Thing at a Time

Each increment changes one logical thing. Don't mix feature + refactor + config.

### Rule 3: Keep It Compilable

After each increment: `cargo build` must pass, existing tests must pass.

### Rule 4: Safe Defaults

```rust
pub fn ingest(event: Event, opts: IngestOpts) -> Result<()> {
    let notify = opts.notify.unwrap_or(false);
    ...
}
```

### Rule 5: Rollback-Friendly

- Schema migrations must be reversible
- Additive changes (new files, functions) easy to revert
- Don't delete and replace in same commit — separate them

## Increment Checklist

- [ ] Change does one thing completely
- [ ] `cargo test` passes
- [ ] `cargo build` passes
- [ ] New functionality works as expected
- [ ] Committed with descriptive message

## Rationalizations vs Reality

| Rationalization | Reality |
|---|---|
| "Test it all at end" | Bugs compound. Bug in Slice 1 makes Slices 2-5 wrong |
| "Faster to do all at once" | Until something breaks and 500 changed lines hide cause |
| "Changes too small to commit" | Small commits free. Large commits hide bugs |
| "Refactor small enough to include" | Mixed refactors + features make both harder to review |

## Red Flags

- More than 100 lines without running tests
- Multiple unrelated changes in one increment
- "Let me just quickly add this too" scope expansion
- Build or tests broken between increments
