---
name: planning-and-task-breakdown
description: >
  Breaks work into ordered, verifiable tasks. Use when you have a spec or clear
  requirements and need implementable units. Use when a task feels too large,
  when estimating scope, or when parallel work is possible.
---

# Planning and Task Breakdown

Decompose work into small, verifiable tasks with explicit acceptance criteria.

## Process

### Step 1: Enter Plan Mode

Read-only. Read spec, scan codebase, map dependencies. Do NOT write code.

### Step 2: Map Dependency Graph

```
Data structs + types
    |
    +-- Core logic functions
    |       |
    |       +-- Storage trait + impl
    |       |       |
    |       |       +-- HTTP handlers
    |       |
    |       +-- Validation + error types
    |
    +-- Tests + fixtures
```

Build foundations first, work up.

### Step 3: Slice Vertically

```
# BAD (horizontal)
Task 1: Build all structs
Task 2: Build all storage impls
Task 3: Build all HTTP handlers

# GOOD (vertical)
Task 1: Ingest event (struct + parse + unit test)
Task 2: Store event (storage trait + impl + integration test)
Task 3: Query events (query fn + handler + test)
```

Each slice delivers working, testable functionality.

### Step 4: Write Tasks

```markdown
## Task [N]: [Short title]

**Acceptance criteria:**
- [ ] [Specific, testable condition]

**Verification:**
- [ ] `cargo test src/collector.rs`
- [ ] Manual check: [what to verify]

**Dependencies:** [Task numbers or "None"]
**Files:** [src/, tests/]
**Size:** [S: 1-2 files | M: 3-5 files | L: 5+ → break down further]
```

### Step 5: Order and Checkpoint

After every 2-3 tasks, add checkpoint:

```markdown
## Checkpoint: After Tasks 1-3
- [ ] `cargo test` passes
- [ ] `cargo clippy -- -D warnings` passes
- [ ] Core flow works end-to-end
- [ ] Review with human before proceeding
```

## Task Sizing

| Size | Files | Example |
|------|-------|---------|
| **XS** | 1 | Add validation to existing function |
| **S** | 1-2 | New module function + test |
| **M** | 3-5 | Feature slice (struct + logic + handler) |
| **L** | 5-8 | Multi-component feature |
| **XL** | 8+ | Too large — break down further |

Break task down when:
- Can't describe acceptance criteria in 3 bullet points
- Touches two independent subsystems
- Task title contains "and" (sign of two tasks)

## Parallelization

- **Safe**: independent feature slices, tests for existing features, docs
- **Sequential**: schema migrations, shared state, dependency chains
- **Needs coordination**: features sharing module API (define API first)

## Rationalizations vs Reality

| Rationalization | Reality |
|---|---|
| "Figure it out as I go" | How you get rework. 10 min planning saves hours |
| "Tasks are obvious" | Write them down. Explicit tasks surface hidden deps |
| "Planning is overhead" | Planning IS the task. Implementation without plan is typing |

## Red Flags

- Starting implementation without written task list
- Tasks that say "implement the feature" without criteria
- No verification steps in plan
- All tasks XL-sized
- No checkpoints between phases
