---
name: breakdown-epic-pm
description: >
  Generates Epic-level Product Requirements Documents. Use when creating epic
  PRDs, scoping features, translating high-level ideas into structured specs,
  or preparing work for engineering handoff.
---

# Breakdown Epic (PRD)

Translate high-level ideas into Epic-level PRDs. Single source of truth for engineering. Feeds task breakdown.

## When to Use

- Scoping new feature or initiative
- Translating discovery findings into buildable specs
- Preparing work for engineering handoff
- Creating "what" before "how"

## When NOT to Use

- Problem space unexplored (`discovery-process` first)
- Requirements clear and small (`spec-driven-development`)
- Breaking existing epic into tasks (`planning-and-task-breakdown`)

## Process

### Step 1: Gather Context

Collect before writing:
- **Epic idea**: high-level description from user or discovery output
- **Target users**: who is this for?
- **Evidence**: customer quotes, analytics, discovery artifacts
- **Constraints**: technical, timeline, regulatory

Missing info → ask clarifying questions. Don't fill gaps with assumptions.

### Step 2: Write PRD

Save to `specs/{epic-name}.md` using template below.

### Step 3: Review

Walk through each section:
- Success metrics measurable and time-bound?
- "Out of Scope" explicit enough to prevent scope creep?
- Functional requirements map to user journeys?

### Step 4: Hand Off

Once approved:
1. `planning-and-task-breakdown` — decompose into tasks
2. `spec-driven-development` — technical specification

## PRD Template

```markdown
# Epic: [Epic Name]

## Goal

**Problem:** [What user problem or business need this addresses. 3-5 sentences.]

**Solution:** [How this epic solves the problem at high level.]

**Impact:** [Expected outcomes or metrics to improve.]

## User Personas

[Target user(s) — role, goals, pain points.]

## User Journeys

[Key workflows enabled. Step-by-step.]

## Requirements

### Functional

- [What epic must deliver, business perspective]
- [Specific, testable requirements]

### Non-Functional

- [Performance, security, accessibility, data privacy constraints]

## Success Metrics

- [KPI 1: metric, target, timeframe]
- [KPI 2: metric, target, timeframe]

## Out of Scope

- [What is NOT included — explicit to prevent scope creep]

## Business Value

[High / Medium / Low] — [Brief justification]

## Open Questions

- [Unresolved decisions needed before or during build]
```

## Output

Save to `specs/{epic-name}.md` (lowercase, hyphenated slug).

## Related Skills

- `discovery-process` — validate problem/solution before PRD
- `spec-driven-development` — technical spec after PRD
- `planning-and-task-breakdown` — decompose epic into tasks

## Anti-Patterns

| Anti-Pattern | Fix |
|---|---|
| PRD without evidence | Ground every requirement in user research or data |
| Vague metrics ("improve UX") | Make metrics specific and time-bound |
| No "Out of Scope" section | Always define boundaries explicitly |
| PRD as implementation spec | PRD defines *what* and *why*, not *how* |
| Writing PRD after building | PRD comes before code |

## Red Flags

- Epic has no identified target user
- No success metrics defined
- "Out of Scope" section empty
- PRD saved outside `specs/`
- Jumping to task breakdown without PRD review
