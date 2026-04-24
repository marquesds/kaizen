# Part 7 — Optional: proxy, sync, telemetry

These features are **opt-in**. Nothing leaves your machine until you configure it. Redaction applies before outbound paths — see [concepts.md](../concepts.md#redact).

## LLM HTTP proxy

Run Kaizen in front of an Anthropic-style API so model calls become `EventSource::Proxy` rows in the same SQLite store.

```bash
kaizen proxy run
kaizen proxy run --listen 127.0.0.1:3847
kaizen proxy run --upstream https://api.anthropic.com
```

Client setup, `X-Kaizen-Session`, and `[proxy]` keys: [llm-proxy.md](../llm-proxy.md).

**CLI-only:** there is no MCP tool for `proxy run`.

## Sync (redacted ingest)

When `[sync]` in config points at an endpoint, `kaizen sync` flushes the **redacted** outbox.

```bash
kaizen sync run --once
kaizen sync run
kaizen sync status
```

Contract for operators: [ingest-contract.md](../ingest-contract.md).

## Pluggable telemetry

PostHog, Datadog, OTLP, or `dev` sinks mirror the same redaction story as sync.

```bash
kaizen telemetry configure
kaizen telemetry print-effective-config
```

Templates and env resolution: [config.md](../config.md#telemetry) and [usage.md](../usage.md#kaizen-telemetry).

**CLI-only:** telemetry subcommands are not MCP tools.

## Exercise

1. Read [llm-proxy.md](../llm-proxy.md) and decide whether proxy fits your threat model (loopback default, API key forwarding).
2. With sync **disabled** (empty endpoint), run `kaizen sync status` and confirm the outbox is idle or empty.
3. Run `kaizen telemetry print-effective-config` and verify no secrets are printed (redacted resolution only).

**Next:** [Part 8 — MCP](08-mcp.md)
