# Part 9 — Housekeeping: gc and completions

## Retention and gc

Sessions older than `[retention].hot_days` may be removed **automatically** after a rescan (at most once per 24h), or you can prune explicitly:

```bash
kaizen gc
kaizen gc --days 14
kaizen gc --vacuum
```

`hot_days = 0` disables automatic pruning; you can still run `gc` with **`--days`** set. See [config.md](../config.md#retention) and [usage.md](../usage.md#kaizen-gc).

**CLI-only:** no MCP tool for `gc`.

## Shell completions

```bash
kaizen completions bash
kaizen completions zsh
kaizen completions fish
kaizen completions elvish
kaizen completions powershell
```

Redirect stdout to the path your shell expects. Examples: [usage.md](../usage.md#kaizen-completions).

## ingest hook (rare)

Hooks call this; you normally do not. For debugging:

```bash
kaizen ingest hook --source cursor < event.json
```

## You are done

You now have a path through **every** major Kaizen surface: setup, observe, interpret, metrics, retro, experiments, optional outbound plumbing, MCP, and retention.

- Reference: [usage.md](../usage.md)
- Story: [telemetry-journey.md](../telemetry-journey.md)
- Index: [tutorial README](README.md)
