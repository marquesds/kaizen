# Phase 4 — Tiered Storage

Hot mmap log + warm SQLite + cold Parquet, queried via embedded DuckDB.
Solves analytics speed + retention-as-`rm` + cheap multi-workspace.
Estimated effort: 3-4 sprints. Risk: high (schema migration).

## Scope

### 4.1 Tier definitions

| Tier | Medium | Window | Read pattern |
|---|---|---|---|
| HOT | mmap rkyv log + redb KV | last ~24h or 1 GB | recent events, live tail |
| WARM | SQLite (existing schema, slimmed) | metadata + open spans + 7d | TUI list, joins |
| COLD | Parquet daily partitions | older than 7d, retained per config | analytics, retro, search |

Boundary policy configurable: `[storage] hot_max_bytes = "1GB"`,
`[storage] cold_after_days = 7`, `[storage] retention_days = 90`.

### 4.2 Hot log

`src/store/hot_log.rs` — append-only file `hot/log.bin`:

```
[u64 magic][u32 version]
[record]*    where record = [u32 len][rkyv-archived Event][u32 crc]
```

mmap'd; appender holds last page in RAM. fsync batched (100ms or
4 KB). Reader uses zero-copy `rkyv::access`.

Index in `hot/index.redb`:

```
sessions:  session_id  -> SessionMeta { first_offset, last_offset, last_seq }
seq_idx:   (session_id, seq) -> offset
```

redb is pure-Rust, MVCC, no global lock. Single writer (daemon),
multi-reader.

### 4.3 Warm tier (SQLite, slimmed)

Drop from SQLite:
- raw `events` (only metadata kept: count, first/last ts, agent).
- closed `tool_spans` older than warm window (move to Parquet).
- `repo_snapshots` history (keep current + prev only).

Keep in SQLite:
- `sessions` (cardinality low, lots of joins).
- `session_repo_binding`, `experiment_*`, `session_feedback`.
- open `tool_spans` (active sessions).
- registry tables.

Migration: `kaizen migrate v2` reads existing `kaizen.db`, writes
hot log + initial Parquet partitions, slims SQLite. Reversible via
`kaizen migrate v1` (writes events back from log + Parquet).

### 4.4 Cold tier (Parquet)

Daily partitions:
`cold/events/YYYY-MM-DD.parquet`
`cold/tool_spans/YYYY-MM-DD.parquet`
`cold/file_facts/YYYY-MM-DD.parquet` (snapshot per day)

Schema versioned via Parquet KV metadata `kaizen_schema_v=N`.
Writer: arrow-rs RecordBatch builder; daemon flushes hot → cold at
midnight (configurable).

Retention: `find cold/ -mtime +RETENTION -delete`. Bounded, atomic,
no DELETE cascade.

### 4.5 Query engine: embedded DuckDB

`src/store/query.rs` — DuckDB connection with Parquet glob view:

```sql
CREATE VIEW events AS
  SELECT * FROM read_parquet('~/.kaizen/workspaces/*/cold/events/*.parquet')
  UNION ALL
  SELECT * FROM warm_events;  -- via SQLite scanner extension or shadow table
```

`retro`, `summary`, `metrics`, `--all-workspaces`: all become DuckDB
queries. SQLite stays for transactional single-row reads (TUI detail).

DuckDB benchmarks suggest 10-100× SQLite for analytics scans on
columnar data — independently verified in Phase 4 bench.

### 4.6 Live-tail unification

Tail readers (TUI, MCP) read hot log via daemon push. After
hot→cold flush, push a `Compaction { ranges }` delta so clients
invalidate caches.

### 4.7 Sync outbox

Today `sync_outbox` rows live in SQLite. Move to redb queue under
`hot/outbox.redb` — append + drain pattern. Sync worker drains;
on success, deletes range. Crash-safe via redb txn.

## Acceptance criteria

| Metric | After P3 | Target P4 |
|---|---|---|
| `retro --days 30` (1M events) | ~5s | <1s |
| `--all-workspaces` (10 ws, 10M events) | ~50s | <2s |
| Disk per 1M events | ~1.5 GB | <300 MB (Parquet zstd) |
| Retention 30→7d | minutes (DELETE) | seconds (`rm`) |

Migration test: `tests/perf/migrate_v2.rs` — round-trip 100k
sessions, byte-equality check on derived facts.

Quint: `specs/event-log-hot.qnt` covers append, rotate, replay.

## Rollback

`kaizen migrate v1` reverses to SQLite-only. Supported for one
minor release window. Cold Parquets remain readable for offline
inspection.

## Risk

- Schema drift between SQLite, Parquet, hot log. Mitigated by single
  source-of-truth `Event` struct + codegen for all three writers.
- DuckDB binary size +~15 MB. Acceptable; gated by feature flag
  `analytics-duckdb` (default on; off for minimal build).
- rkyv `unsafe` — contained to `hot_log.rs`, fuzz-tested
  (`cargo fuzz`).
- Time skew across partitions — UTC enforced; `ts_ms` invariant.

## Out of scope

Search (Phase 5). Network sync changes (use existing redacted batch).

## Dependencies

Requires Phase 3 (daemon owns the writes). Phase 2 projector emits
to all three tiers via sink trait.
