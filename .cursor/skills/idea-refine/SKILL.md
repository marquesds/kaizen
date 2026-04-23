---
name: idea-refine
description: >
  Refines ideas through structured divergent and convergent thinking. Use when
  you have a rough concept, need to ideate, stress-test a plan, or explore
  variations before committing to implementation.
---

# Idea Refine

Refine raw ideas into sharp, actionable concepts through three phases.

## Process

### Phase 1: Understand and Expand (Divergent)

1. **Restate** idea as crisp "How Might We" problem statement
2. **Ask 3-5 sharpening questions** (no more):
   - Who is this for, specifically?
   - What does success look like?
   - What are real constraints?
   - What's been tried before?
   - Why now?
3. **Generate 5-8 idea variations** using lenses:
   - **Inversion**: what if we did opposite?
   - **Constraint removal**: what if time/tech weren't factors?
   - **Simplification**: 10x simpler version?
   - **Combination**: merge with adjacent idea?
   - **Expert lens**: what would domain experts find obvious?

Inside codebase: scan existing architecture, patterns, constraints. Ground variations in what actually exists.

### Phase 2: Evaluate and Converge

1. **Cluster** resonating ideas into 2-3 distinct directions
2. **Stress-test** each against:
   - **User value**: painkiller or vitamin?
   - **Feasibility**: hardest part?
   - **Differentiation**: would someone switch from current solution?
3. **Surface hidden assumptions**: what you're betting is true but haven't validated

Be honest, not supportive. Weak idea → say so with specificity.

### Phase 3: Sharpen and Ship

Produce markdown one-pager:

```markdown
# [Idea Name]

## Problem Statement
[One-sentence "How Might We" framing]

## Recommended Direction
[Chosen direction and why — 2-3 paragraphs max]

## Key Assumptions to Validate
- [ ] [Assumption 1 — how to test it]
- [ ] [Assumption 2 — how to test it]

## MVP Scope
[Minimum version that tests core assumption]

## Not Doing (and Why)
- [Thing 1] — [reason]
- [Thing 2] — [reason]

## Open Questions
- [Questions needing answers before building]
```

Save to `docs/ideas/[idea-name].md` after user confirmation.

## Anti-Patterns

- Don't generate 20+ shallow variations (5-8 considered ones better)
- Don't skip "who is this for"
- Don't be yes-machine for weak ideas
- Don't produce plan without surfacing assumptions
- Don't ignore existing codebase constraints

## Red Flags

- No "How Might We" problem statement
- Target user and success criteria undefined
- No assumptions surfaced before committing
- Missing "Not Doing" list
- Jumping to output without running phases
