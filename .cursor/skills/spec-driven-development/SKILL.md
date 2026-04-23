---
name: spec-driven-development
description: >
  Creates specs before coding. Use when starting a new feature, project, or
  significant change. Use when requirements are unclear, ambiguous, or when
  change touches multiple modules.
---

# Spec-Driven Development

Write structured spec before any code. Code without spec is guessing.

## When to Use

- Starting new feature or module
- Change touches multiple files or modules
- About to make architectural decision
- Requirements ambiguous

## Gated Workflow

```
SPECIFY → PLAN → TASKS → IMPLEMENT
Each gate requires human review before advancing.
```

### Phase 1: Specify

Surface assumptions immediately:

```
ASSUMPTIONS:
1. Rust binary crate (no web framework yet)
2. tokio async runtime
3. Storage backend TBD (in-memory for now)
→ Correct me now or I proceed with these.
```

Write spec covering six areas:

1. **Objective** — what, why, who, success criteria
2. **Commands** — `cargo test`, `cargo run`, `cargo build`
3. **Structure** — modules, traits, types to create/modify
4. **Code Style** — one real Rust snippet showing pattern
5. **Testing Strategy** — `#[test]`, `#[tokio::test]`, unit vs integration
6. **Boundaries**:
   - **Always**: `cargo test` before commits, follow naming conventions
   - **Ask first**: new dependencies, interface changes, module restructuring
   - **Never**: commit secrets, skip clippy, remove failing tests

Reframe vague requirements as success criteria:

```
REQUIREMENT: "Make ingestion faster"
SUCCESS CRITERIA:
- Single ingest < 1ms (criterion benchmark)
- Batch of 100 events < 50ms
- No allocation in hot path
→ Are these right targets?
```

### Phase 2: Plan

Generate technical plan: modules, types, traits, dependencies, order, risks.

### Phase 3: Tasks

Break into discrete tasks (see `planning-and-task-breakdown` skill).

### Phase 4: Implement

Execute one task at time using TDD. Update spec when decisions change.

## Spec Template

```markdown
# Spec: [Feature Name]

## Objective
[What, why, acceptance criteria]

## Tech Stack
Rust edition 2024, tokio, [other deps]

## Commands
cargo test, cargo run, cargo build, cargo clippy

## Structure
[Modules, traits, types, files]

## Code Style
[Example snippet]

## Testing Strategy
[#[test], #[tokio::test], unit/integration split]

## Boundaries
- Always: [...]
- Ask first: [...]
- Never: [...]

## Success Criteria
[Specific, testable conditions]
```

## Rationalizations vs Reality

| Rationalization | Reality |
|---|---|
| "Simple, no spec needed" | Simple tasks still need acceptance criteria. Two lines fine |
| "Write spec after coding" | That's documentation, not specification |
| "Spec slows us down" | 15-min spec prevents hours of rework |
| "Requirements will change" | That's why spec is living document |

## Red Flags

- Starting code without written requirements
- Architectural decisions without documenting them
- Implementing features not in any spec
- Skipping spec because "it's obvious"
