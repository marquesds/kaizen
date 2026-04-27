# Phase 5 — Tantivy Full-Text Search

Replace linear scan of prompts/payloads with BM25 index. Independent
of Phase 3/4 — can ship after Phase 0.
Estimated effort: 1 sprint. Risk: medium (index lifecycle).

## Scope

### 5.1 Index schema

`search/` directory under workspace root. Tantivy schema:

```
session_id    STRING (stored, indexed)
ts_ms         I64    (stored, fast)
agent         STRING (stored, fast)
kind          STRING (fast)
text          TEXT   (indexed, BM25, stored=false)
path          STRING (indexed)   // file mentioned in tool_use
skill         STRING (indexed)
```

Documents: one per `Event` with extractable text (Message, ToolCall,
ToolResult). No payload bodies stored — only indexed.

### 5.2 Writer

In daemon (Phase 3) — projector emits `SearchDoc` alongside
`SpanClosed`. Writer commits in batches of 1000 docs or 5s.

Standalone mode (no daemon): in-process writer with single-process
lock; flushed on `kaizen` exit or explicit `kaizen search reindex`.

### 5.3 Reader

```bash
kaizen sessions search "deadlock" \
  --since 7d --agent claude-code --limit 50
kaizen sessions search 'path:src/store/sqlite.rs' --kind tool_use
kaizen sessions search 'skill:caveman AND tokens>5000'
```

Query syntax: tantivy default (Lucene-ish). Output: session_id, ts,
score, snippet (highlighted).

MCP tool: `mcp/search_sessions` — same query, structured response
for agent consumption.

### 5.4 Reindex

`kaizen search reindex` — drops index, rebuilds from event log
(hot + Parquet). Bench: 1M events ~2 minutes single-thread; tantivy
parallel build option for >10M.

### 5.5 Privacy / redaction

Same redactor as `sync` runs before tantivy ingest. Secrets, env
vars, absolute paths normalized. Index is local-only; never leaves
disk unless user runs `kaizen search export`.

## Acceptance criteria

| Metric | Before | Target |
|---|---|---|
| `sessions search "X"` over 1M events | 30s+ (grep scan) | <50ms |
| Index size per 1M events | n/a | <200 MB |
| Index update latency post-event | n/a | <2s |

Test: `tests/spec/search.qnt` covers index/replay parity (queryable
↔ persisted events).

## Rollback

Drop `search/` directory; `kaizen sessions search` falls back to
`grep`-style scan over events with deprecation warning.

## Risk

- Index corruption on crash — tantivy commits are atomic; worst case
  reindex from log.
- Disk growth — capped by retention (drop docs older than
  `retention_days`).
- Query syntax learning curve — ship `--help` examples + cookbook
  in `docs/usage.md`.

## Out of scope

Vector / semantic search (no embeddings — out of localhost-light
constraint). Could be added later behind `search-semantic` feature
flag if user demand.

## Dependencies

None hard. Best paired with Phase 4 (cold tier feeds reindex
cheaply); usable standalone after Phase 0.
