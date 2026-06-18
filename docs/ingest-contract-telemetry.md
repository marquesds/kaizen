# Telemetry Ingestion

Return to the [ingest contract entrypoint](ingest-contract.md).

## `POST /v1/tool-spans`

Derived client-side from `tool_call` + `tool_result` + hook timing.
Exact-or-null only. No estimated token or lead-time backfill.

```json
{
  "team_id": "kaizen-eng",
  "workspace_hash": "blake3:5f3c...",
  "project_name": "kaizen",
  "spans": [
    {
      "session_id_hash": "blake3:9a2b...",
      "span_id_hash": "blake3:ab12...",
      "tool": "shell",
      "status": "done",
      "started_at_ms": 1745344800000,
      "ended_at_ms": 1745344801200,
      "lead_time_ms": 1200,
      "tokens_in": 1402,
      "tokens_out": 312,
      "reasoning_tokens": 91,
      "cost_usd_e6": 8400,
      "path_hashes": ["blake3:77aa..."]
    }
  ]
}
```

## `POST /v1/repo-snapshots`

Chunked code facts. No raw paths, symbols, commits, or file contents.

```json
{
  "team_id": "kaizen-eng",
  "workspace_hash": "blake3:5f3c...",
  "project_name": "kaizen",
  "snapshots": [
    {
      "snapshot_id_hash": "blake3:de91...",
      "commit_hash": "blake3:f0aa...",
      "indexed_at_ms": 1745344800000,
      "dirty": false,
      "chunk_index": 0,
      "chunk_total": 2,
      "file_facts": [
        {
          "path_hash": "blake3:aa11...",
          "language": "rust",
          "bytes": 4200,
          "loc": 130,
          "sloc": 108,
          "complexity_total": 18,
          "max_fn_complexity": 6,
          "symbol_count": 12,
          "import_count": 4,
          "fan_in": 3,
          "fan_out": 4,
          "churn_30d": 5,
          "churn_90d": 9,
          "authors_90d": 2,
          "last_changed_ms": 1745000000000
        }
      ],
      "edges": [
        {
          "from_hash": "blake3:1a1a...",
          "to_hash": "blake3:2b2b...",
          "kind": "DEPENDS_ON",
          "weight": 1
        }
      ]
    }
  ]
}
```

## `POST /v1/workspace-facts`

One batch per outbox flush of kind `workspace_facts`. Same transport as other
batched `POST` routes (Bearer, gzip, idempotency key, `202` / `409` / `413` /
`429` semantics). Body shape matches the client `WorkspaceFactsBatchBody`:
`team_id`, `workspace_hash`, optional `project_name`, and a `facts` array of
objects with `skill_slugs` and `rule_slugs` (Blake3-hashed identifiers, not raw
paths, unless your redaction policy allowlists cleartext).

```json
{
  "team_id": "kaizen-eng",
  "workspace_hash": "blake3:5f3c...",
  "project_name": "kaizen",
  "facts": [
    {
      "skill_slugs": ["blake3:aa11...", "blake3:bb22..."],
      "rule_slugs": ["blake3:cc33..."]
    }
  ]
}
```

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
