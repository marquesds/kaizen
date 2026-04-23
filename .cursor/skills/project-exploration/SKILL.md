---
name: project-exploration
description: >
  Docs-first project exploration. Use when orienting to codebase, starting
  a new session, onboarding to unfamiliar area, or when user asks "how
  does X work" or "where is Y". Starts from docs/ then drills into source.
---

# Project Exploration

Start from docs, drill into source. Never guess what you can read.

## Quick Start

1. Read `docs/structure.md` — purpose, stack, directory layout
2. Read doc matching task area (see routing table)
3. Follow references into source
4. Read source before modifying

## Doc Routing Table

| I need to understand... | Read first |
|---|---|
| Project purpose, stack, layout | `docs/structure.md` |
| Module graph, data flow, boundaries | `docs/architecture.md` |
| Data structs, relationships, invariants | `docs/datamodel.md` |
| Crates, config, env vars | `docs/config.md` |
| Conventions, design patterns | `docs/patterns.md` |

## Exploration Workflow

### Phase 1: Orient (docs)

```
Read docs/structure.md
  → Understand project, stack, directory layout
  → Identify which module/crate task touches
```

### Phase 2: Narrow (task-specific doc)

```
Read matching doc from routing table
  → Learn public API, key modules, data flow
  → Note referenced source files
```

### Phase 3: Drill (source)

```
Read source files referenced by docs
  → Start with module (e.g. src/collector.rs)
  → Then types, traits, or handlers as needed
  → Read related test file for expected behavior
```

### Phase 4: Pattern Match

```
Before implementing, find existing example:
  → Search for similar pattern in codebase
  → Use that as template, not imagination
```

## Multi-Area Tasks

Read docs in order:
1. `docs/structure.md` (always first)
2. Primary area doc
3. Adjacent area docs as needed
4. Source files last

## When Docs Are Missing or Stale

1. Read source directly (module → types → tests)
2. Read tests for behavioral documentation
3. Flag gap — update or create relevant doc after task

## Anti-Patterns

| Anti-Pattern | Fix |
|---|---|
| Jumping straight to source | Read relevant doc first |
| Reading all docs at once | Read only what task needs |
| Guessing module structure | Check `docs/structure.md` |
| Ignoring test files | Tests document expected behavior |
| Not updating stale docs | Fix docs you find wrong as you go |
