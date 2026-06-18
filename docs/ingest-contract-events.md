# Event Ingestion

Return to the [ingest contract entrypoint](ingest-contract.md).

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
  "project_name": "kaizen",
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
      "tool_call_id": "toolu_01",
      "tokens_in": 1402,
      "tokens_out": 312,
      "reasoning_tokens": 91,
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

### Event `kind` values

Clients send snake_case `kind` per event. Accepted values include `tool_call`,
`tool_result`, `message`, `error`, `cost`, `hook`, and `lifecycle` (behavior
telemetry; `payload` carries a `type` discriminator). Additive: servers should
accept unknown kinds as opaque or map them to `message` per team policy.
