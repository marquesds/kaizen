# Usage

CLI reference. All commands accept `--workspace <path>` (default: cwd).

## `kaizen init`

Idempotent workspace setup. Writes `.kaizen/config.toml`, patches agent
hooks, installs the retro skill. Re-running is safe; originals back up
under `.kaizen/backup/`.

## `kaizen sessions`

```bash
kaizen sessions list           # all sessions in workspace
kaizen sessions show <id>      # full detail: events, tools, cost
```

## `kaizen summary`

Roll-up of count, total USD, by-agent, by-model across all ingested
sessions.

## `kaizen insights`

Activity by day, top tools, recent sessions.

## `kaizen metrics`

Smart metrics over a trailing window.

```bash
kaizen metrics --days 7
kaizen metrics --json
kaizen metrics index --force   # rebuild repo snapshot + Ladybug sidecar
```

## `kaizen tui`

Ratatui-based live session browser. List + detail view, live-tail.

## `kaizen retro`

Weekly heuristic retro. Writes `.kaizen/reports/<iso-week>.md`.

```bash
kaizen retro --days 7
kaizen retro --dry-run          # print Markdown, no file write
kaizen retro --json             # machine-readable
kaizen retro --force            # overwrite this week's report
```

Heuristics: see [retro.md](retro.md). Tuning: see
[retro-tuning.md](retro-tuning.md).

## `kaizen proxy run`

Local HTTP forwarder for Anthropic-style APIs. Records [`EventSource::Proxy` events](concepts.md)
in `.kaizen/kaizen.db` and honors `[proxy]` in config (see [config](config.md), [llm-proxy](llm-proxy.md)).

```bash
kaizen proxy run
kaizen proxy run --listen 127.0.0.1:9000
kaizen proxy run --upstream https://api.anthropic.com
```

## `kaizen ingest hook`

Reads a hook event from stdin and appends to the store. Wired by
`kaizen init`; rarely called directly.

```bash
kaizen ingest hook --source cursor < event.json
kaizen ingest hook --source claude < event.json
```

## `kaizen sync`

Flush redacted outbox to configured ingest endpoint.

```bash
kaizen sync run                 # long-running loop
kaizen sync run --once          # single flush
kaizen sync status              # outbox depth + last flush
```

Contract: [ingest-contract.md](ingest-contract.md).

## `kaizen telemetry`

Optional pluggable sinks (PostHog, Datadog, OTLP, `dev`) that receive the same redacted batches as Kaizen sync. Configure `[[telemetry.exporters]]` in `~/.kaizen/config.toml` (or workspace); see [config.md](config.md#telemetry).

```bash
kaizen telemetry configure              # append an exporter template (interactive)
kaizen telemetry print-effective-config # redacted: which fields resolve from env vs TOML
```

## `kaizen mcp`

Model Context Protocol server over stdio — full CLI parity for agents (Cursor, Claude Code, Goose, OpenCode, Copilot, and so on) without shelling to `kaizen`. Host config examples and tool behavior: [mcp.md](mcp.md).

## `kaizen exp`

Experiments v0.

```bash
kaizen exp new --name add-skill \
  --hypothesis "skill cuts tokens" \
  --change "add .cursor/skills/x" \
  --metric tokens_per_session \
  --bind git --duration-days 14 --target-pct -10

kaizen exp list
kaizen exp status <id>
kaizen exp tag <id> --session <sid> --variant treatment
kaizen exp report <id>          # markdown with bootstrap CI
kaizen exp report <id> --json
kaizen exp conclude <id>
```

Metrics: `tokens_per_session`, `cost_per_session`, `success_rate`,
`tool_loops`, `duration_minutes`, `files_per_session`. Details:
[experiments.md](experiments.md).
