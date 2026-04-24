# Part 4 — Repo intelligence: metrics and refresh

`kaizen metrics` connects sessions to **your repository**: hot files, slow tools (p95), token-heavy tools, and related facts. It answers different questions than `kaizen summary` (which is spend and volume first).

## Default report

```bash
kaizen metrics --days 7
kaizen metrics --json
kaizen metrics --all-workspaces
kaizen metrics --refresh
```

Use **`--refresh`** when you care that transcript-derived events are up to date before metrics runs. Use **`--force`** when you want indexing work even if fingerprints look unchanged (see below).

## Rebuild the repo snapshot and graph sidecar

```bash
kaizen metrics index
kaizen metrics index --force
```

Indexing rebuilds the **repo snapshot** and **code graph** sidecar used for file- and graph-level queries. If retro or metrics look stale after large refactors, an index pass is the usual fix.

**Insight:** If `summary --json` already shows `hotspot` / `slowest_tool`, that comes from a metrics pass tied to the summary path; `metrics` itself is the full report.

## Exercise

1. Run `kaizen metrics --days 7` on an active repo; note the hottest path and slowest tool.
2. Run `kaizen metrics index --force`, then `kaizen metrics --days 7` again and confirm outputs are consistent with your expectations after a big change.
3. Compare mentally: one sentence you would tell your team from `summary` vs one from `metrics`.

**Next:** [Part 5 — Retro](05-retro.md)
