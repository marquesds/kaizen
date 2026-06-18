# Ingest Privacy and Server Guidance

Return to the [ingest contract entrypoint](ingest-contract.md).

## Anonymization Layer (client-side, before send)

Raw kept local in SQLite. Only the redacted projection is POSTed.

| Field | Treatment |
|---|---|
| `workspace_path` | `blake3(team_salt + abs_path)` → `workspace_hash` |
| `git_remote_url` | normalized + hashed with same salt |
| `session_id` | `blake3(team_salt + session_id)` |
| `tool_span_id`, `snapshot_id`, `commit` | hashed with same salt |
| `user`, `git_email` | dropped entirely |
| `file_path` (in payload) | replaced with `<repo-relative-hash>:<basename-class>` |
| `tool span path list` | `path_hashes[]` only |
| `repo edges / symbols` | hashed ids only |
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
- Retention and archival are server-owned policies; the local client does not
  prescribe a storage engine.
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
