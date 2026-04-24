# Part 3 — Interpret: insights and guidance

This part is about **patterns** in how you use agents: activity over time, dominant tools, and whether skills and Cursor rules show up in **tool payloads** (path references).

## Insights dashboard

```bash
kaizen insights
kaizen insights --refresh
kaizen insights --all-workspaces
```

You get activity by day, frequent tools, recent sessions, and a short teaser of “guidance”-style hints. It is a good **daily** command when you want a narrative feel without running full repo metrics.

## Guidance (skills and rules)

```bash
kaizen guidance
kaizen guidance --days 14
kaizen guidance --json
kaizen guidance --refresh
```

**Guidance** estimates adoption of `.cursor/skills/...` and `.cursor/rules/*.mdc` when those paths appear in ingested tool arguments. Silent injection (rules applied without a path string in the payload) is **not** counted — that is intentional so the signal matches “explicitly referenced in tool use.”

**Insight:** Use guidance to answer “are we actually **using** the skills we ship, or only **shipping** them?”

## MCP note

`kaizen guidance` is **CLI-only** today. Agents that need this output should run the shell command or ask you to paste results. See [mcp.md](../mcp.md).

## Exercise

1. Run `kaizen insights` before and after a busy coding day; compare tool counts.
2. Add a skill under `.cursor/skills/` and reference it in a tool call; run `kaizen guidance --json` and locate your skill in the output.
3. If you use multiple repos, try `insights --all-workspaces` once.

**Next:** [Part 4 — Metrics](04-metrics.md)
