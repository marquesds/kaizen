---
name: session-budget
trigger: tool call count hits 25, context hits 50%, or event count hits 100
---
# Session Budget

## Why

One session hit $3.34 / 358 events. Budget caps prevent runaway cost.
Cache hit 92.7% — preserve by not bloating context.

## Checkpoints

| Signal | Action |
|---|---|
| 25 tool calls reached | Stop. Summarize learnings. Decide path. |
| Context >50% full | Consider compact or subagent split. |
| 100 events in session | Hard stop. Must choose: compact / spawn / split PR. |
| Context >75% full | Refuse new work without explicit user override. |

## At Each Checkpoint

1. **Summarize** — what did we learn? what changed? what's left?
2. **Decide** — one of:
   - `compact`: context window compress, continue same session
   - `spawn`: hand remaining work to subagent with summary
   - `split`: commit what's done, open new session for remainder
3. **Act** — don't ask. Pick the right option and state it clearly.

## Compact vs Spawn vs Split

- **Compact**: task continues, just clearing old context. Low overhead.
- **Spawn**: task is independent enough for subagent. Cleanest context.
- **Split**: work is shippable as-is. Commit, start fresh.

## Override

User can say "keep going" or "ignore budget" to override the 75% gate.
Log the override. Don't repeat the warning in same session.

## Reading This Skill

Read when: you notice tool call count climbing, context filling, or session
feel sluggish. Don't wait for the gate to fire — read this proactively.
