# Ingest Contract — Sync Daemon → Server

This page remains the stable entrypoint for the ingest contract. Detailed
payloads live in focused documents:

| Document | Scope |
|---|---|
| [Event ingestion](ingest-contract-events.md) | Primary event batches, responses, idempotency, batching, and event kinds |
| [Telemetry ingestion](ingest-contract-telemetry.md) | Tool spans, repository snapshots, workspace facts, sessions, and experiment observations |
| [Privacy and server guidance](ingest-contract-privacy.md) | Client-side anonymization, server expectations, and design parallels |

## Hook payload (optional fields)

Workspace hooks that call `kaizen ingest hook` may include extra keys on
**SessionStart** JSON:

| Field | Type | Purpose |
|-------|------|---------|
| `pid` | positive integer | OS process id of the agent host to sample when `[collect.system_sampler].enabled` is true |
| `ppid` | positive integer | Reserved; not read in v1 |

These fields are optional; omit `pid` to skip the system sampler. See
[system-telemetry.md](system-telemetry.md).

---

The sections below define the only surface the future server (separate
repo) must implement. Lets the server be swapped without touching
clients. PostHog-inspired: event-first, stateless ingest, batched,
idempotent, project-scoped.

## Versioning

- Path-prefixed: `/v1/...`. Additive within version (new optional fields
  only). Breaking changes go to `/v2/`.
- Server MUST reject unknown required fields with `400` and a
  machine-readable code.
- Client sends `X-Kaizen-Client: kaizen/<semver>` for telemetry.
- Batched event/tool/repo/workspace bodies may include optional
  `project_name`, derived from GitHub origin repo name or workspace folder.
  `workspace_hash` remains the stable join key.

## Endpoints

| Method | Path | Purpose |
|---|---|---|
| `POST` | `/v1/events` | Batched event ingestion (primary) |
| `POST` | `/v1/tool-spans` | Batched per-tool latency + token spans |
| `POST` | `/v1/repo-snapshots` | Batched code-fact snapshot chunks |
| `POST` | `/v1/workspace-facts` | Batched skill/rule slug discovery (redacted; hashed in payload) |
| `POST` | `/v1/sessions` | Upsert session metadata (lifecycle pings) |
| `POST` | `/v1/experiments/:id/observation` | Variant assignment ping |
| `GET`  | `/v1/health` | Liveness; returns server version + accepted schema versions |
| `GET`  | `/v1/config` | Server-driven client overrides (sample rate, batch limits) |

## `POST /v1/events`

See the complete [event ingestion contract](ingest-contract-events.md#post-v1events).

### Idempotency

See [batch-level and event-level idempotency](ingest-contract-events.md#idempotency).

### Batching

See [client batching limits and server overrides](ingest-contract-events.md#batching).

### Event `kind` values

See [accepted event kinds and unknown-kind handling](ingest-contract-events.md#event-kind-values).

## `POST /v1/tool-spans`

See the [tool span payload](ingest-contract-telemetry.md#post-v1tool-spans).

## `POST /v1/repo-snapshots`

See the [repository snapshot payload](ingest-contract-telemetry.md#post-v1repo-snapshots).

## `POST /v1/workspace-facts`

See the [workspace facts payload](ingest-contract-telemetry.md#post-v1workspace-facts).

## `POST /v1/sessions`

See the [session lifecycle payload](ingest-contract-telemetry.md#post-v1sessions).

## `POST /v1/experiments/:id/observation`

See the [experiment observation payload](ingest-contract-telemetry.md#post-v1experimentsidobservation).

## Anonymization Layer (client-side, before send)

See the complete [client-side anonymization contract](ingest-contract-privacy.md#anonymization-layer-client-side-before-send).

## Server Expectations (informational, not built here)

See [server storage and tenancy expectations](ingest-contract-privacy.md#server-expectations-informational-not-built-here).

## PostHog Parallels

See the [PostHog-inspired contract shapes](ingest-contract-privacy.md#posthog-parallels).
