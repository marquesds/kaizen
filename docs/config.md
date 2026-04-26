# Config

Config is TOML. **Paths:**

| File | Role |
|------|------|
| `<workspace>/.kaizen/config.toml` | Per-repo (checked into VCS or local only) |
| `~/.kaizen/config.toml` | Per-user; use for secrets and machine-wide defaults |

**Load order:** the workspace and user files are both read; `src/core/config.rs` merges them as follows.

- **`[sync]`, `[proxy]`, `[telemetry]`:** field-by-field merge. Workspace is applied first, then the user file overwrites (non-empty strings, set numbers, and so on). Nested **`[telemetry.query]`** and **`[telemetry.query.identity_allowlist]`** merge the same way (per-key; see below).
- **`[scan]`:** `roots` — if the user file sets a non-default `roots` list, that wins; otherwise the workspace’s `[scan].roots` is used. `min_rescan_seconds` merges the same way (user non-default wins, else workspace).
- **`[retention]`:** field-by-field merge: for each of `hot_days` and `warm_days`, if the user file value differs from the schema default, the user value wins; otherwise the workspace value is kept.
- **`[sources]`:** `cursor` and `tail.*` merge like `[retention]`: for each key, if the user file’s value still equals the type default, the workspace value is kept; otherwise the user value wins. Use the user file for machine-wide toggles; workspace file for repo-specific overrides.
- **`[collect.outcomes]`, `[collect.system_sampler]`:** same pattern as `[retention]` (user non-default wins per key; otherwise workspace).

LLM HTTP proxy: [llm-proxy.md](llm-proxy.md).

## Environment variables (general)

| Var | Default | Purpose |
|-----|---------|--------|
| `RUST_LOG` | (unset) | Log filter for the `tracing` stack (e.g. `info`, `kaizen=debug`) |
| `HOME` | (required) | Resolves `~` paths and the user config location |
| `KAIZEN_HOME` | (unset) | Overrides the machine-local Kaizen home used for the workspace registry and other non-workspace files |
| `OPENCLAW_STATE_DIR` | `~/.openclaw` | Override OpenClaw state directory (used by `tail.openclaw` and tests) |
| `OPENCLAW_HOME` | (unset) | Secondary override for OpenClaw home (resolved before `~/.openclaw` fallback) |

## Machine-local registry

Kaizen records known workspace roots in **`$KAIZEN_HOME/machine.db`** (default **`~/.kaizen/machine.db`**) — a small SQLite file with one row per canonical path (first seen, last seen, last `kaizen init`, optional `git` remote, and so on).

- **Registration:** any command that resolves a workspace (default cwd or `--workspace`) **upserts** that path. **`kaizen init`** also records the workspace after hook setup (even before a local `.kaizen/kaizen.db` exists).
- **Legacy file:** if **`~/.kaizen/workspaces.json`** is present from an older build, it is imported once and renamed to **`workspaces.json.migrated`**.

When you pass **`--all-workspaces`** (or MCP `all_workspaces: true`), Kaizen loads that list, ensures the current workspace is included, **keeps a path** if it still exists on disk and it **either** has a local **`.kaizen/kaizen.db`** **or** appears in the machine registry (e.g. only ran `init`), then opens each per-workspace DB that exists and merges results. The seed workspace is always kept when in scope. See [usage.md](usage.md) for which commands support this.

## `[scan]`

| Key | Default | Purpose |
|-----|---------|--------|
| `roots` | `["~/.cursor/projects"]` | Transcript index roots (Cursor projects layout) |
| `min_rescan_seconds` | `300` | Minimum seconds between full transcript rescans when a command is already in refresh mode (`--refresh` on the CLI or `refresh=true` over MCP) |

## `[retention]`

| Key | Default | Purpose |
|-----|---------|--------|
| `hot_days` | `30` | Local SQLite keeps sessions started within the last **hot_days** days. Older sessions and dependent rows are removed when auto-prune runs (after a rescan, at most once per 24h) or when you run `kaizen gc`. **`0`** disables automatic pruning. |
| `warm_days` | `90` | Reserved for future tiered retention; not used for local purge today. |

## `[sources]`

| Key | Default | Purpose |
|-----|---------|--------|
| `cursor.enabled` | `true` | Tier-1 Cursor transcript discovery |
| `cursor.transcript_glob` | `*/agent-transcripts` | Glob under each scan root |
| `tail.goose` | `true` | Tail Goose JSONL / paths (see [concepts](concepts.md#collection)) |
| `tail.openclaw` | `true` | Tail OpenClaw sessions from `~/.openclaw/agents/*/sessions/` |
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

Optional fan-out to local and third-party sinks: **`file`** (append-only NDJSON under the workspace), PostHog, Datadog, OTLP, or `dev` tracing, with the same redaction as Kaizen sync. The `file` sink needs no extra Cargo feature; others may (for example `telemetry-posthog`); see [Cargo features](../Cargo.toml) and [usage](usage.md#kaizen-telemetry).

| Key | Default | Purpose |
|-----|---------|--------|
| `fail_open` | `true` | If `true`, exporter errors are ignored; if `false`, flush fails when any secondary sink errors |

### `[telemetry.query]`

Remote read-back (provider pull) and cache policy. OTLP is **export only**; it is not a query authority. Defaults keep pull disabled and identity fields hashed/omitted unless allowlisted.

| Key | Default | Purpose |
|-----|---------|--------|
| `provider` | `none` | `none` \| `posthog` \| `datadog` — single query authority for pull when implemented. |
| `cache_ttl_seconds` | `3600` | Treat cached provider rows as fresh for this many seconds (unless the user forces refresh). With `--source provider` or `mixed` on read commands, Kaizen may skip `telemetry pull` while the cache is fresh; use `--refresh` to force a pull. |

`kaizen summary`, `insights`, `metrics`, `guidance`, and `retro` accept `--source` with values `local` (default), `provider`, or `mixed`. The PostHog/Datadog **exporters** still fan out the same redacted sync batches; Datadog is mapped to [Logs v2](https://docs.datadoghq.com/api/latest/logs/) per expanded item. `kaizen telemetry print-schema` lists canonical event names. `kaizen telemetry doctor` checks provider health when configured; OTLP has no query path in v1.

### `[telemetry.query.identity_allowlist]`

When `true`, the corresponding field may be emitted in **cleartext** on outbound / canonical telemetry for that key; when `false` (default), omit or hash. Keys: `team`, `workspace_label`, `runner_label`, `actor_kind`, `actor_label`, `agent`, `model`, `env`, `job`, `branch`.

**Exporters** are `[[telemetry.exporters]]` tables with `type = "file" | "posthog" | "datadog" | "otlp" | "dev" | "none"`. For `file`, optional `path` (relative paths resolve against the workspace root) defaults to **`.kaizen/telemetry.ndjson`**. The `kaizen telemetry configure` command appends a template block to `~/.kaizen/config.toml`.

**Credential resolution (per exporter):** standard env vars are preferred, with `KAIZEN_`-prefixed fallbacks in some cases, for example:

| Sink | Common env vars |
|------|-----------------|
| file | (none) |
| PostHog | `POSTHOG_API_KEY`, `POSTHOG_HOST` (or `KAIZEN_POSTHOG_*`) |
| Datadog | `DD_API_KEY`, `DD_SITE` (or `KAIZEN_DD_*`) |
| OTLP | `OTEL_EXPORTER_OTLP_ENDPOINT` (or `KAIZEN_OTEL_EXPORTER_OTLP_ENDPOINT`) |

Redacted effective resolution: `kaizen telemetry print-effective-config`. Implementation: `src/telemetry/resolve.rs`.

## `[collect.outcomes]`

Post-stop test (and optional lint) measurement. Ingest spawns a detached `kaizen outcomes measure` after `Stop` when `enabled = true`. See [outcomes.md](outcomes.md).

| Key | Default | Purpose |
|-----|---------|--------|
| `enabled` | `false` | When `true`, run worker after each session `Stop` hook |
| `test_cmd` | `cargo test --quiet` | Shell command in session workspace (Unix: `/bin/sh -c`) |
| `timeout_secs` | `600` | Per-command wall-clock limit; process is killed on expiry |
| `lint_cmd` | (unset) | Optional e.g. `cargo clippy -- -D warnings` |

## `[collect.system_sampler]`

Per-process CPU/RSS samples for the agent PID from `SessionStart`. Ingest spawns `kaizen __sampler-run` when `enabled` and `pid` is present in the hook JSON. Stops on `Stop` via `.kaizen/sampler-stop/<session_id>`. See [system-telemetry.md](system-telemetry.md). Windows: not supported in v1.

| Key | Default | Purpose |
|-----|---------|--------|
| `enabled` | `false` | When `true`, start sampler on `SessionStart` if `pid` is set |
| `sample_ms` | `2000` | Sleep between samples (minimum 100 ms) |
| `max_samples_per_session` | `3600` | Hard cap per session |

## `[eval]`

LLM-as-a-Judge evaluations. Disabled by default; set `enabled = true` to activate.

```toml
[eval]
enabled      = false                        # must opt in
endpoint     = "https://api.anthropic.com"  # Anthropic-compatible base URL
api_key      = ""                           # falls back to ANTHROPIC_API_KEY env var
model        = "claude-haiku-4-5-20251001"  # judge model
rubric       = "tool-efficiency-v1"         # built-in rubric id
batch_size   = 20                           # max sessions per eval run
min_cost_usd = 0.01                         # skip sessions cheaper than this
```

**API key resolution:** `api_key` is checked first; if empty, `ANTHROPIC_API_KEY` is used. Put the key in `~/.kaizen/config.toml` to keep it out of the repo.

**Merge:** `api_key` — non-empty user value wins. All other fields: user non-default wins, else workspace value.
