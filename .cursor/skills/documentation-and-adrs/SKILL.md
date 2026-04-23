---
name: documentation-and-adrs
description: >
  Documentation standards and Architecture Decision Records. Use when making
  architectural decisions, changing APIs, shipping features, or when codebase
  needs its "why" documented. Covers ///, //!, cargo doc, ADRs.
---

# Documentation and ADRs

Document WHY, not WHAT. Code shows what; docs explain why.

## What to Document

### Always

- **Architecture decisions** — why X over Y (ADR)
- **Public module APIs** — `///` with preconditions, postconditions
- **Non-obvious constraints** — why something must be done specific way
- **Module purpose** — `//!` with module's role

### Never

- What code obviously does ("inserts record")
- Implementation details code already expresses
- Temporary workarounds without removal plan

## `///` and `//!` Doc Comments

```rust
//! Session storage module.
//!
//! All session operations are atomic. Storage is append-only.

/// Create session in store.
///
/// # Preconditions
/// `session_id` must be unique. Duplicate → `Err(StoreError::Duplicate)`.
///
/// # Postconditions
/// Session persisted and queryable immediately.
///
/// # Errors
/// `StoreError::Duplicate` if `session_id` already exists.
///
/// # Examples
/// ```
/// let store = MemoryStore::new();
/// store.create(session)?;
/// ```
pub fn create(&self, session: Session) -> Result<(), StoreError> {
    ...
}
```

Run `cargo doc --open` to verify docs render correctly.

## Architecture Decision Records (ADRs)

Store in `docs/decisions/`. One file per decision.

### ADR Template

```markdown
# ADR-001: Use Append-Only Event Log for Session Storage

## Status
Accepted

## Context
Session telemetry requires audit trail.
Mutable storage loses history on update.

## Decision
All session events stored as append-only log.
No in-place updates. Queries reconstruct state from log.

## Consequences
- Full audit trail preserved
- Query cost higher (reconstruct from log)
- Simpler write path (no conflict resolution)
```

### When to Write ADR

- Choosing between technologies or approaches
- Establishing pattern team must follow
- Decision hard or expensive to reverse
- Deviating from common convention

## Architecture Docs

Maintain in `docs/`:

- `docs/structure.md` — purpose, stack, directory layout
- `docs/architecture.md` — module graph, data flow, external boundaries
- `docs/datamodel.md` — entities, relationships, domain model
- `docs/decisions/` — ADR files

Keep each doc under 200 lines. Split when it grows.

## Rationalizations vs Reality

| Rationalization | Reality |
|---|---|
| "Code is self-documenting" | Code shows what, not why. Context lost without docs |
| "Docs get outdated" | Outdated docs = maintenance problem, not reason to skip |
| "ADRs are overhead" | 10-min ADR saves hours of "why did we do this?" |

## Red Flags

- Public functions without `///`
- Modules without `//!`
- Architectural decisions without written rationale
- Docs narrating code line-by-line
- `cargo doc` warnings ignored
