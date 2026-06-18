# Telemetry

[Back to CLI index](usage.md).

## `kaizen telemetry`

Pluggable sinks receive the same redacted batches as Kaizen sync. The default
build ships PostHog, Datadog, and OTLP exporters. The `dev` tracing sink remains
opt-in through `--features telemetry-dev`. The file sink defaults to
`~/.kaizen/projects/<slug>/telemetry.ndjson`.

Configuration lives in `[[telemetry.exporters]]` under
`~/.kaizen/config.toml`. See [config.md](config.md#telemetry).

`telemetry configure` resolves credentials from flags or provider environment
variables, runs a health check, and writes configuration only after success.
Datadog rejects Application Keys where an API Key is required. Datadog and
PostHog configuration also sets the query authority when no explicit
`[telemetry.query]` table exists.

`telemetry test` sends one synthetic redacted event to every configured sink.
Telemetry-only flows generate `~/.kaizen/local_salt.hex` with mode `0600` when
`[sync].team_salt_hex` is empty.

Datadog pull uses Logs Search v2. It reads `DD_API_KEY` from exporter config
before environment fallback. `DD_APP_KEY` remains environment-only. PostHog
pull is not implemented; OTLP is export-only.

```bash
kaizen telemetry configure --type datadog \
  --site us5.datadoghq.com --non-interactive
kaizen telemetry configure --type posthog \
  --host https://us.i.posthog.com --non-interactive
kaizen telemetry configure --type file --path telemetry.ndjson
kaizen telemetry configure --type otlp --endpoint http://127.0.0.1:4318

kaizen telemetry test
kaizen telemetry print-effective-config
kaizen telemetry doctor
kaizen telemetry pull --days 1
kaizen telemetry push

kaizen telemetry tail
kaizen telemetry tail --no-follow
kaizen telemetry tail --file /tmp/t.ndjson
kaizen telemetry tail --json
```

`telemetry push` replays SQLite events through every configured exporter
without sending them to a Kaizen ingest endpoint.
