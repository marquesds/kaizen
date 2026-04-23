---
name: agentlens
description: >
  Local observability for coding-agent sessions. Use when reviewing what an
  agent did, debugging failed sessions, checking token/cost spend, comparing
  approaches across sessions, or investigating daily agent activity.
---

# AgentLens — Agent Session Observability

Inspect sessions before guessing what went wrong. One local surface for traces from Cursor, Claude Code, Codex, Gemini, Pi, and OpenCode.

## When to Use

- Session failed or produced unexpected results
- Reviewing what tools agent called and in what order
- Checking token usage and cost
- Comparing two approaches to same task
- Daily/weekly activity review across all agents
- Debugging why session stalled or looped

## Quick Reference

### CLI

```bash
agentlens summary                          # overview of all indexed sessions
agentlens sessions list --limit 20         # recent sessions
agentlens session latest --show-tools      # last session with tool calls
agentlens sessions events latest --follow  # live-stream events from latest
```

### Browser UI

```bash
agentlens --browser    # opens http://127.0.0.1:8787
```

Browser UI provides:
- **Daily Activity** — timeline of parallel sessions, pastel event coloring
- **Week Heatmap** — click day to drill into timeline
- **Trace Inspector** — event-level detail, tool calls, errors, token counts

### Deep-Link Specific File

```bash
node -e 'const p=process.argv[1]; console.log(
  `http://localhost:8787/trace-file/${Buffer.from(p).toString("base64url")}`
)' "/absolute/path/to/trace.log"
```

## How Cursor Sessions Are Discovered

Auto-discovers at:

```
~/.cursor/projects/**/agent-transcripts/*.txt
```

No extra config needed — traces appear automatically.

## Configuration

Config at `~/.agentlens/config.toml`. Read/update via CLI:

```bash
agentlens config get
agentlens config set scan.intervalSeconds 1.5
agentlens config set scan.includeMetaDefault true
```

### Key Sections

| Section | Controls |
|---|---|
| `[scan]` | Refresh cadence, status freshness TTLs |
| `[retention]` | Hot/warm/cold in-memory trace policy |
| `[sources.*]` | Discovery roots, include/exclude globs |
| `[redaction]` | Key/value redaction for secrets |
| `[cost]` | Model pricing tables, estimation policy |

### Tuning Tips

- Keep `roots` narrow to reduce discovery cost
- Use `excludeGlobs` for noisy/archive directories
- Disable unused source profiles to speed refresh

## Session Activity States

| State | Meaning |
|---|---|
| `running` | Unmatched tool call or recent activity |
| `waiting_input` | Explicit wait markers or text-pattern fallback |
| `idle` | No active signal or TTL timeout |

## Workflow Integration

### After Failed Session

1. `agentlens session latest --show-tools` — see what happened
2. `agentlens --browser` for trace inspector if CLI not enough
3. Check error events, looping tool calls, unexpected results

### Cost Review

1. `agentlens summary` — aggregate token/cost data
2. Browser UI for per-session cost breakdowns
3. Compare sessions on similar tasks to spot inefficiency

### Comparing Approaches

Open two sessions in browser UI trace inspector side by side.
Compare tool call sequences, token usage, outcomes.

## Anti-Patterns

| Anti-Pattern | Fix |
|---|---|
| Guessing what agent did | Run `agentlens session latest` first |
| Ignoring cost data | Check `agentlens summary` periodically |
| Debugging without traces | Open browser UI for full event detail |
| Stale index after config change | Run `agentlens summary` to verify counts |
