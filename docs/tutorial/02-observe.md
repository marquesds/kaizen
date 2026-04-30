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

The JSON shape includes rollups by agent and model, total cost, and when available a **hotspot** file and **slowest_tool** hint — useful when you want one object for dashboards or agents. If sessions exist but rollup cost is **$0.00**, Kaizen may add optional **`cost_note`** in JSON (and a short note in plain text) because stored events have no **`cost_usd_e6`**; see [usage.md — When cost rollup is zero](usage.md#cost-shows-zero).

## Data source: local, provider, or mixed

Most of this tutorial assumes the default **`--source local`**: numbers come from the workspace’s local SQLite store (and the usual transcript rescan rules with `--refresh`).

When you have **[sync]** identity in config (`team_id`, `team_salt_hex`, …) *and* a **[telemetry.query](https://github.com/marquesds/kaizen/blob/main/docs/config.md#telemetryquery) provider** (PostHog or Datadog) configured, you can ask read commands to fold in **provider-pulled events** cached under `remote_events` in the same DB:

| Flag | Meaning |
|------|--------|
| `--source local` | Default. Local store only. |
| `--source provider` | Prefer aggregates from imported remote events when the cache has rows; falls back to local if identity or cache is missing. |
| `--source mixed` | Combine local stats with remote-derived aggregates (same team/workspace hash as sync). |

Examples:

```bash
kaizen summary --source mixed
kaizen summary --source provider --refresh
```

**`--refresh`** with `provider` or `mixed` can **force a telemetry pull** when the provider cache TTL says the data is stale (see `cache_ttl_seconds` in [config.md](../config.md#telemetryquery)).

Details and limitations (what merges vs stays local) follow the same story as [Part 3](03-insights-guidance.md), [Part 4](04-metrics.md), and [Part 7](07-proxy-sync-telemetry.md).

## Live session browser

```bash
kaizen tui
```

This is the richest **interactive** way to explore turns and tools. It is **not** available over MCP; hosts that need graphs should shell to the CLI or use list/summary/metrics tools.

The TUI loads only the visible session and event rows, then fetches more as you
scroll. Agent filtering uses a case-insensitive prefix and runs in SQLite, so
large workspaces stay responsive instead of loading every session into memory.

## Machine-wide aggregation (optional)

If you use Kaizen in several repos on one machine:

```bash
kaizen sessions list --all-workspaces
kaizen summary --all-workspaces
```

Kaizen merges databases for workspaces listed in the machine registry (`~/.kaizen/machine.db`, see [config.md](../config.md#machine-local-registry)).

## Exercise

1. With an empty store, run `sessions list` — expect no rows or only old data until you have ingested sessions.
2. After one agent run in the wired repo, run `sessions list` again (add `--refresh` if you know the transcript just rotated).
3. Open `kaizen tui`, pick a session, and watch how detail differs from `sessions show`.
4. If you later enable sync + a query provider, try `summary --source mixed` once and compare to plain `summary`.

**Next:** [Part 3 — Insights and guidance](03-insights-guidance.md)
