# Part 5 — Improve: retro

Retro turns recent telemetry into **ranked bets**: what to change to save tokens or time, with effort estimates. It is **heuristic** — no LLM inside the engine. Deep dive: [retro.md](../retro.md); tuning thresholds: [retro-tuning.md](../retro-tuning.md).

## Generate a report

```bash
kaizen retro --days 7
```

By default this writes **Markdown** under `.kaizen/reports/<iso-week>.md`.

Useful flags:

```bash
kaizen retro --dry-run    # print Markdown, no file
kaizen retro --json       # structured Report on stdout
kaizen retro --force      # overwrite this week’s file
kaizen retro --refresh    # rescan transcripts first
kaizen retro --source mixed   # local events + remote_events in the window (with sync + query provider)
```

**`--source`** follows the same three-way model as `summary` / `insights` (see [Part 2](02-observe.md#data-source-local-provider-or-mixed)). For **`provider`**, the retro pipeline can use cached remote events when local identity fields match; **`mixed`** unions remote events with local ones in the time window. Configure **[telemetry.query](https://github.com/marquesds/kaizen/blob/main/docs/config.md#telemetryquery)** for pulls.

## Read the output like a staff engineer

Look for **high impact / reasonable effort** bets first. Each bet ties back to observed patterns (tool loops, file churn, model mix). If a bet feels wrong, your next step is often **tuning** or gathering more weeks of data — not ignoring retro entirely.

## Exercise

1. Run `kaizen retro --dry-run --days 7` and skim the Markdown.
2. Open `.kaizen/reports/` after a normal run; compare to `--json` output for the same window.
3. Pick one bet and map it to a concrete repo change (rule, skill, or CI guard).

**Next:** [Part 6 — Experiments](06-experiments.md)
