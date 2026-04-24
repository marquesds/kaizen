# Part 2 — Observe: sessions, summary, TUI

## Cache-first reads

Most read commands use the **local** `.kaizen/kaizen.db` immediately. They do not rescan every agent transcript directory on every invocation — that keeps the CLI snappy. When you need the latest lines from disk, pass **`--refresh`** (`-r`). See [usage.md](../usage.md) and `[scan].min_rescan_seconds` in [config.md](../config.md).

## List sessions

```bash
kaizen sessions list
```

Try also:

```bash
kaizen sessions list --json
kaizen sessions list --refresh
```

**`--json`** is for scripts and MCP-shaped tooling. **`--refresh`** forces a full transcript rescan (subject to min interval).

## Show one session

```bash
kaizen sessions show <id>
```

This prints **metadata** for that session (agent, model, times, status, `trace_path`) — not the full tool/event stream. For a browsable, live view, use the TUI below.

## Cost and volume rollup

```bash
kaizen summary
kaizen summary --json
```

The JSON shape includes rollups by agent and model, total cost, and when available a **hotspot** file and **slowest_tool** hint — useful when you want one object for dashboards or agents.

## Live session browser

```bash
kaizen tui
```

This is the richest **interactive** way to explore turns and tools. It is **not** available over MCP; hosts that need graphs should shell to the CLI or use list/summary/metrics tools.

## Machine-wide aggregation (optional)

If you use Kaizen in several repos on one machine:

```bash
kaizen sessions list --all-workspaces
kaizen summary --all-workspaces
```

Kaizen merges databases registered in `~/.kaizen/workspaces.json` (see [config.md](../config.md#machine-local-registry)).

## Exercise

1. With an empty store, run `sessions list` — expect no rows or only old data until you have ingested sessions.
2. After one agent run in the wired repo, run `sessions list` again (add `--refresh` if you know the transcript just rotated).
3. Open `kaizen tui`, pick a session, and watch how detail differs from `sessions show`.

**Next:** [Part 3 — Insights and guidance](03-insights-guidance.md)
