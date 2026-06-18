# ADR 010: SQLite-Only Local Storage

## Status

Accepted

Supersedes [ADR 006: Performance Redesign](006-perf-redesign.md).

## Date

2026-06-17

## Context

ADR 006 proposed a tiered local store after assuming Kaizen had crossed
100,000 sessions. Current real workspaces remain far below that assumption.
Meanwhile, SQLite paging, indexes, cache-first reads, the incremental projector,
and the optional single-writer daemon removed the observed interactive
bottlenecks without adding another source of truth.

The tiered design added DuckDB, Arrow, Parquet, an rkyv memory-mapped hot log,
unsafe code, storage-specific configuration, and a reversible migration command.
That complexity increased binary size, build time, maintenance cost, and upgrade
risk while solving scale Kaizen does not currently have.

Current release checks produced these measurements on real project data:

| Gate | Measured result |
|---|---:|
| Release binary | 31.49 MiB (33,020,112 bytes) |
| Clean release build | 80.05 s |
| Session list p95 | 6.23 ms |
| Session search p95 | 15.03 ms |
| Session detail p95 | 8.70 ms |
| Summary p95 | 58.20 ms |
| Web snapshot p95 | 49.25 ms; 13,191 bytes |
| Idle daemon | No CPU-time growth over 30 s; 13.6 MiB RSS |
| Post-Web footprint | 68.2 MiB physical; 89.3 MiB peak over 50 snapshots |

These measurements are decision gates for the current workload, not universal
latency guarantees across all hardware or data sets.

## Decision

SQLite WAL is the only canonical local store for sessions, events, and derived
rows. The project data path remains
`~/.kaizen/projects/<slug>/kaizen.db`.

Kaizen keeps the performance work that proved useful:

- SQL paging and indexes for bounded reads.
- Bounded SQLite defaults: an 8 MiB page cache and 32 MiB memory map.
- SQL-bounded visualization windows: Web reads 30 sessions and 40 selected
  events, spans, and files; TUI reads 100 sessions and 200 detail rows.
- Cache-first commands with explicit refresh.
- Incremental projection of tool spans and derived facts.
- Optional daemon ownership of writes, with direct SQLite mode retained.
- Rebuildable Tantivy search and GraphQLite code-graph sidecars.

Kaizen removes DuckDB, Arrow, Parquet storage, the rkyv hot log, and
`kaizen migrate`. Legacy hot-event and cold-partition files are not read. A
legacy `hot/outbox.redb` is imported once into SQLite and archived so pending
sync rows survive the transition. Existing SQLite event data remains
authoritative; the former migration also kept a `kaizen.db.v1.bak` copy.

Retention deletes old sessions and dependent rows from SQLite through automatic
pruning or `kaizen gc`. There is no local cold archive.

## Alternatives

- Keep tiered storage dormant behind features. Rejected because dormant code,
  dependencies, tests, and migration paths still impose maintenance cost.
- Keep only Parquet export. Rejected because no current query or portability
  requirement justifies another persisted representation.
- Move to a server database. Rejected because it breaks the local-first,
  single-binary operating model.

## Trade-offs

Kaizen gains a smaller binary, simpler installation, one crash-recovery model,
fewer native dependencies, and one source of truth. Kaizen gives up local
columnar archives and any claim that the present design is validated at
100,000 active sessions.

## Consequences

- Users no longer choose or migrate between local storage formats.
- Old tiered artifacts can be deleted after confirming `kaizen.db` is intact.
- A missing SQLite database is not reconstructed from old hot or cold files.
- Performance work should target queries, indexes, paging, and projection
  before introducing another storage engine.
- Reconsider this decision only when representative real-data p95 measurements
  repeatedly exceed twice the gates above, or active data approaches the old
  100,000-session assumption.
