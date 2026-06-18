# Part 6 — Improve: experiments

Experiments let you **bind** a hypothesis to commits or manual session tags, then compare a metric with bootstrap confidence intervals. Full reference: [experiments.md](../experiments.md).

## Create an experiment

```bash
kaizen exp new --name add-skill \
  --hypothesis "skill cuts tokens" \
  --change "add .cursor/skills/my-skill" \
  --metric tokens_per_session \
  --bind git \
  --duration-days 14 --target-pct=-10
```

Metrics include `tokens_per_session`, `cost_per_session`, `success_rate`, `tool_loops`, `duration_minutes`, `files_per_session`.

## Operate the lifecycle

```bash
kaizen exp list
kaizen exp status <id>
kaizen exp tag <id> --session <sid> --variant treatment
kaizen exp report <id>
kaizen exp report <id> --json
kaizen exp report <id> --refresh   # ingest changed transcript tails if the store may be stale
kaizen exp conclude <id>
```

Use **`tag`** when binding is manual; use **`git`** binding when control and treatment are defined by commits (see product doc for your workflow).

## Exercise

1. Create a **draft** experiment with a hypothesis you actually believe.
2. Run `exp list` and `exp status` until you understand state transitions.
3. When you have enough tagged sessions, run `exp report` and read the CI around the median delta.

**Next:** [Part 7 — Proxy, sync, telemetry](07-proxy-sync-telemetry.md)
