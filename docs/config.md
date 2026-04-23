# Config

Config is TOML. **Paths:**

| File | Role |
|------|------|
| `<workspace>/.kaizen/config.toml` | Per-repo (checked into VCS or local only) |
| `~/.kaizen/config.toml` | Per-user; use for secrets and machine-wide defaults |

**Load order:** the workspace and user files are both read; `src/core/config.rs` merges them as follows.

- **`[sync]`, `[proxy]`, `[telemetry]`:** field-by-field merge. Workspace is applied first, then the user file overwrites (non-empty strings, set numbers, and so on).
- **`[scan].roots`:** if the user file sets a non-default `roots` list, that wins; otherwise the workspace’s `[scan].roots` is used.
- **`[sources]`, `[retention]`:** the merged value comes only from the user file (`~/.kaizen/config.toml`); if that file is missing, schema defaults are used. Per-repo `[sources]` / `[retention]` in the workspace `config.toml` are **not** applied today — put `sources` and `retention` in `~/.kaizen/config.toml` when you need to change them.

LLM HTTP proxy: [llm-proxy.md](llm-proxy.md).

## Environment variables (general)

| Var | Default | Purpose |
|-----|---------|--------|
| `RUST_LOG` | (unset) | Log filter for the `tracing` stack (e.g. `info`, `kaizen=debug`) |
| `HOME` | (required) | Resolves `~` paths and the user config location |

## `[scan]`

| Key | Default | Purpose |
|-----|---------|--------|
| `roots` | `["~/.cursor/projects"]` | Transcript index roots (Cursor projects layout) |

## `[retention]`

| Key | Default | Purpose |
|-----|---------|--------|
| `hot_days` | `30` | Hot tier days |
| `warm_days` | `90` | Warm tier days |

## `[sources]`

| Key | Default | Purpose |
|-----|---------|--------|
| `cursor.enabled` | `true` | Tier-1 Cursor transcript discovery |
| `cursor.transcript_glob` | `*/agent-transcripts` | Glob under each scan root |
| `tail.goose` | `true` | Tail Goose JSONL / paths (see [concepts](concepts.md#collection)) |
| `tail.opencode` | `true` | Tail OpenCode agent data |
| `tail.copilot_cli` | `true` | Tail GitHub Copilot CLI sessions |
| `tail.copilot_vscode` | `true` | Tail VS Code Copilot chat exports |

## `[sync]`

| Key | Default | Purpose |
|-----|---------|--------|
| `endpoint` | `""` | If empty, sync is disabled (no outbox flush) |
| `team_token` | `""` | Bearer or team token (keep in user config, not in git) |
| `team_id` | `""` | Team id for ingest |
| `team_salt_hex` | `""` | 64 hex chars (32 bytes) for id hashing; prefer user config only |
| `events_per_batch_max` | `500` | Max events per upload batch |
| `max_body_bytes` | `1_000_000` | Max batch body size |
| `flush_interval_ms` | `10_000` | Background flush interval |
| `sample_rate` | `1.0` | 0.0–1.0 sample of events to enqueue |

## `[proxy]`

Local HTTP forwarder for Anthropic-style APIs. Full key list, defaults, and `context_policy` examples: [llm-proxy.md](llm-proxy.md).

## `[telemetry]`

Optional fan-out to third-party sinks (PostHog, Datadog, OTLP, or `dev` tracing) with the same redaction as Kaizen sync. Build features may be required (for example `telemetry-posthog`); see [Cargo features](../Cargo.toml) and [usage](usage.md#kaizen-telemetry).

| Key | Default | Purpose |
|-----|---------|--------|
| `fail_open` | `true` | If `true`, exporter errors are ignored; if `false`, flush fails when any secondary sink errors |

**Exporters** are `[[telemetry.exporters]]` tables with `type = "posthog" | "datadog" | "otlp" | "dev" | "none"`. The `kaizen telemetry configure` command appends a template block to `~/.kaizen/config.toml`.

**Credential resolution (per exporter):** standard env vars are preferred, with `KAIZEN_`-prefixed fallbacks in some cases, for example:

| Sink | Common env vars |
|------|-----------------|
| PostHog | `POSTHOG_API_KEY`, `POSTHOG_HOST` (or `KAIZEN_POSTHOG_*`) |
| Datadog | `DD_API_KEY`, `DD_SITE` (or `KAIZEN_DD_*`) |
| OTLP | `OTEL_EXPORTER_OTLP_ENDPOINT` (or `KAIZEN_OTEL_EXPORTER_OTLP_ENDPOINT`) |

Redacted effective resolution: `kaizen telemetry print-effective-config`. Implementation: `src/telemetry/resolve.rs`.
