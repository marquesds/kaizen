---
name: discovery-process
description: >
  Structured discovery cycle from problem hypothesis to validated solution.
  Use when exploring new features, investigating churn/retention, validating
  strategic initiatives, or running continuous discovery.
---

# Discovery Process

Explore problem spaces, validate solutions before committing to build.
Avoids "build it and they will come" syndrome.

Based on Teresa Torres (_Continuous Discovery Habits_) and Rob Fitzpatrick (_The Mom Test_).

## When to Use

- Exploring new product/feature area
- Investigating retention or churn problems
- Validating strategic initiatives before roadmap commitment
- Continuous discovery (weekly customer touchpoints)

## When NOT to Use

- Well-understood problems (move to execution)
- Stakeholders already committed to solution (address alignment first)
- Tactical bug fixes or technical debt

## Workflow

```
FRAME → RESEARCH → INTERVIEWS → SYNTHESIZE → VALIDATE → DECIDE
Each phase has decision gate before advancing.
```

### Phase 1: Frame (Day 1-2)

Use `idea-refine` skill for divergent/convergent thinking.

1. Write problem hypothesis:
   "We believe [persona] struggles with [problem] because [root cause],
   leading to [consequence]."
2. Define 3-5 research questions
3. Set success criteria: what validates/invalidates the problem?

**Output:** Problem hypothesis, research questions, success criteria.
Save to `docs/ideas/[discovery-name].md`.

**Gate:** Enough context to start research? If NO, gather existing data first
(support tickets, analytics, churn surveys, NPS). +2-3 days.

### Phase 2: Research Planning (Day 3)

1. Write interview guide: 5-7 open-ended questions (Mom Test style)
   - Focus on past behavior, not hypotheticals
   - "Tell me about last time you [experienced this problem]"
   - "How do you currently handle this?"
2. Recruit 5-10 participants from target persona
3. Schedule 45-60 min sessions over 1-2 weeks

**Output:** Interview guide, participant roster, synthesis plan.

### Phase 3: Interviews (Week 1-2)

1. Run 5-10 discovery interviews
2. Capture structured notes per interview:
   - Context (when/where they experience problem)
   - Actions (what they do, step-by-step)
   - Pain points and workarounds
   - Verbatim quotes
3. Review support tickets and analytics in parallel

**Gate:** Reached saturation? (same pain points from 3+ participants)
If NO, schedule 3-5 more. +1 week.

### Phase 4: Synthesize (End Week 2)

1. Affinity map: group insights by theme, count frequency
2. Score each pain point on frequency, intensity, strategic fit (1-5)
3. Rank top 3-5 pain points
4. Update problem hypothesis based on evidence

**Output:** Ranked pain points, customer quotes, validated problem statement.

### Phase 5: Validate Solutions (Week 3)

1. For each top pain point, generate 3 solution options
2. Design cheapest experiment per solution:
   - Concierge test (manually deliver to 10 customers)
   - Prototype test (clickable mockup, 10 users)
   - Landing page test (fake door, measure interest)
   - A/B test (minimal build, 50% of users)
3. Define success criteria per experiment
4. Run experiments (1-2 weeks each)

**Gate:** Experiments validate? If NO, pivot to next solution. +1-2 weeks.

### Phase 6: Decide (End Week 3-4)

**GO / PIVOT / KILL** based on: problem validated, solution validated, strategic fit.

If **GO**:
1. `breakdown-epic-pm` → write epic PRD in `specs/`
2. `planning-and-task-breakdown` → decompose into tasks
3. 30-min readout to stakeholders

If **PIVOT**: return to Phase 5 with next solution.
If **KILL**: document learnings, de-prioritize.

## Timeline

| Track | Duration | Interviews | Experiments |
|-------|----------|------------|-------------|
| Fast | 3 weeks | 5 | 1 |
| Typical | 4 weeks | 7-10 | 1-2 |
| Thorough | 6-8 weeks | 10+ | Multiple rounds |

## Related Skills

- `idea-refine` — Phase 1 divergent/convergent thinking
- `spec-driven-development` — specs before code
- `breakdown-epic-pm` — epic PRDs from validated discoveries
- `planning-and-task-breakdown` — decompose epics into tasks

## Anti-Patterns

| Anti-Pattern | Fix |
|---|---|
| Skip interviews, rely only on analytics | Interview 5-10 customers per cycle |
| Ask leading questions ("Would you use X?") | Mom Test: focus on past behavior |
| 2-3 interviews then declare done | Continue until saturation (3+ same patterns) |
| 6 weeks synthesizing, never ship | Time-box discovery to 3-4 weeks |
| Run discovery once, stop | Continuous: 1 interview/week ongoing |

## Red Flags

- No problem hypothesis before research starts
- Interview questions ask about hypothetical future behavior
- Solution chosen without any experiment
- Discovery artifacts not saved to `docs/ideas/`
- Jumping to Phase 6 without evidence from Phases 3-5
