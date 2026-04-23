# Ingest Contract — Sync Daemon → Server

Stable HTTP API. Defines the only surface the future server (separate
repo) must implement. Lets the server be swapped without touching
clients. PostHog-inspired: event-first, stateless ingest, batched,
idempotent, project-scoped.

## Versioning

- Path-prefixed: `/v1/...`. Additive within version (new optional fields
  only). Breaking changes go to `/v2/`.
- Server MUST reject unknown required fields with `400` and a
  machine-readable code.
- Client sends `X-Kaizen-Client: kaizen/<semver>` for telemetry.

## Endpoints

| Method | Path | Purpose |
|---|---|---|
| `POST` | `/v1/events` | Batched event ingestion (primary) |
| `POST` | `/v1/sessions` | Upsert session metadata (lifecycle pings) |
| `POST` | `/v1/experiments/:id/observation` | Variant assignment ping |
| `GET`  | `/v1/health` | Liveness; returns server version + accepted schema versions |
| `GET`  | `/v1/config` | Server-driven client overrides (sample rate, batch limits) |

## `POST /v1/events`

Headers:

```
Authorization: Bearer <team-token>
Content-Type: application/json
Content-Encoding: gzip
X-Kaizen-Idempotency-Key: <uuid-v7 per batch>
X-Kaizen-Client: kaizen/0.1.0
```

Body:

```json
{
  "team_id": "kaizen-eng",
  "workspace_hash": "blake3:5f3c...",
  "events": [
    {
      "session_id_hash": "blake3:9a2b...",
      "event_seq": 1247,
      "ts_ms": 1745344800123,
      "agent": "cursor",
      "model": "claude-4.6-sonnet",
      "kind": "tool_call",
      "source": "hook",
      "tool": "Edit",
      "tokens_in": 1402,
      "tokens_out": 312,
      "cost_usd_e6": 8400,
      "payload": { "files_in_context": 3, "tool_args_size": 412 }
    }
  ]
}
```

Responses:

| Code | Meaning |
|---|---|
| `202 Accepted` | Batch queued. Returns `{ "received": N, "deduped": M }` when present; clients accept an empty body. |
| `400` | Schema rejection. Returns `{ "code": "...", "field": "...", "message": "..." }`. |
| `401` | Bad token. |
| `409` | Idempotency key replay; safe to ignore. |
| `413` | Batch too large; client should split. |
| `429` | Rate limited; honor `Retry-After`. |

### Idempotency

- Batch-level: `X-Kaizen-Idempotency-Key` deduped server-side for at
  least 24h. Replays return `409` with the original response body.
- Event-level: server uniqueness on
  `(team_id, workspace_hash, session_id_hash, event_seq)`. Duplicates
  silently dropped, counted in `deduped`.

### Batching

Client-side defaults:

- Max **500 events** per batch.
- Max **1 MB** uncompressed body.
- Max **10 s** wait before flushing partial batch.
- Whichever limit hits first.

Server `GET /v1/config` may override these (e.g. `events_per_batch_max:
1000`).

## `POST /v1/sessions`

Lifecycle pings (start, status changes, end). Lightweight, can be
sent outside the main event batch loop. Same auth + idempotency.

Body:

```json
{
  "team_id": "kaizen-eng",
  "workspace_hash": "blake3:5f3c...",
  "session_id_hash": "blake3:9a2b...",
  "agent": "cursor",
  "model": "claude-4.6-sonnet",
  "started_at_ms": 1745344800000,
  "ended_at_ms": null,
  "status": "running"
}
```

## `POST /v1/experiments/:id/observation`

One ping per session classified into an experiment. Lets server-side
analytics aggregate experiments without re-deriving binding from the
event stream.

Body:

```json
{
  "team_id": "kaizen-eng",
  "session_id_hash": "blake3:9a2b...",
  "variant": "treatment",
  "binding_kind": "git_commit",
  "metric_snapshot": { "tokens_per_session": 15902 }
}
```

## Anonymization Layer (client-side, before send)

Raw kept local in SQLite. Only the redacted projection is POSTed.

| Field | Treatment |
|---|---|
| `workspace_path` | `blake3(team_salt + abs_path)` → `workspace_hash` |
| `git_remote_url` | normalized + hashed with same salt |
| `session_id` | `blake3(team_salt + session_id)` |
| `user`, `git_email` | dropped entirely |
| `file_path` (in payload) | replaced with `<repo-relative-hash>:<basename-class>` |
| `tool_args.command` (shell) | command name kept, args matched against secret regex set, tokens replaced with `<REDACTED:type>` |
| `env vars`, `Authorization`, `*_TOKEN`, `*_KEY` | scrubbed by `aho-corasick` set + regex |
| `prompt_text`, `completion_text` | dropped in v0.1; opt-in field for v0.2 with extra redaction pass |

`team_salt` is a 32-byte secret in `~/.kaizen/config.toml`, never sent
upstream. Same salt across team → consistent hashes across devs (workspace
A on my machine and yours map to the same `workspace_hash`).

Quint spec `specs/redaction-completeness.qnt` models the redaction step
(invariants on forbidden markers). OpenAPI subset lives in
`specs/openapi/ingest-v1.yaml`.

## Server Expectations (informational, not built here)

For the future server-repo author:

- Events table partitioned by day, time-ordered.
- Dedup on `(team_id, workspace_hash, session_id_hash, event_seq)`.
- Raw payload stored compressed (zstd recommended, matches client).
- 30-day hot retention default, parquet archive thereafter.
- `team_id` scoping on every query (multi-tenant ready, single-tenant fine).

## PostHog Parallels

Borrowed shapes (kept simple — no Kafka, no ClickHouse mandate):

- **Event-first schema** — every interaction is an event with `ts`,
  `kind`, `payload`. Sessions are a derived view.
- **Stateless ingest** — server can scale horizontally; idempotency
  on the client handles retries.
- **Project / team scoping** — `team_id` on every request.
- **Server-driven config** — `GET /v1/config` lets the server tune
  client batching without a redeploy.
