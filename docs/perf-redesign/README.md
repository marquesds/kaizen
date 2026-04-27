# Performance Redesign

Plan for sub-100ms TUI cold start at 100k+ sessions. Five phases, each
shippable. See [ADR 006](../adr/006-perf-redesign.md) for the decision.

## Goal

Fluid TUI + CLI at any local scale. Hard targets:

| Metric | Today (10k sess) | After P0-P2 | After P4 |
|---|---|---|---|
| `kaizen tui` cold start | ~5-15s | <500ms | <100ms |
| Refresh delta | 500ms poll, full scan | event-driven, <50ms | <16ms |
| Append event latency p99 | ~50ms (rebuild O(n²)) | <5ms | <1ms |
| `retro --days 30` | ~30s | ~5s | <1s |
| `--all-workspaces` (10 ws) | linear N | linear N | sub-linear (DuckDB glob) |

## Phases

| # | File | Status | Depends |
|---|---|---|---|
| 0 | [phase-0-quick-wins.md](phase-0-quick-wins.md) | planned | — |
| 1 | [phase-1-tui-virtualization.md](phase-1-tui-virtualization.md) | planned | 0 |
| 2 | [phase-2-incremental-projector.md](phase-2-incremental-projector.md) | planned | 0 |
| 3 | [phase-3-daemon.md](phase-3-daemon.md) | planned | 2 |
| 4 | [phase-4-tiered-storage.md](phase-4-tiered-storage.md) | planned | 3 |
| 5 | [phase-5-search.md](phase-5-search.md) | planned | 0 |

## Architecture (post-Phase 4)

```
┌────────────────────────────────────────────────────────────┐
│  TUI / CLI / MCP (read-only clients)                        │
│  • virtualized list (viewport + prefetch)                   │
│  • Arc<ArcSwap<Snapshot>> view cache                        │
│  • Arrow IPC over Unix socket OR direct mmap                │
└────────────────────┬────────────────────────────────────────┘
                     │
┌────────────────────▼────────────────────────────────────────┐
│  kaizen-daemon (single writer, multi reader)                │
│  • ingest (hooks, tails, proxy) → mpsc                      │
│  • incremental projector (event → span/file/repo deltas)    │
│  • snapshot publisher (debounced, delta-encoded)            │
│  • DuckDB read-only attach for analytics                    │
│  • tantivy writer + reader (search)                         │
└────────────────────┬────────────────────────────────────────┘
                     │
┌────────────────────▼────────────────────────────────────────┐
│  Storage (per workspace under .kaizen/)                     │
│  • hot/log.bin       append mmap, rkyv, ~24h, 50 B/event    │
│  • hot/index.redb    KV: session_id → offsets, last_seq     │
│  • warm/kaizen.db    SQLite: thin metadata, open spans      │
│  • cold/YYYY-MM-DD.parquet  daily partitions, retention=rm  │
│  • search/           tantivy index of prompts/payloads      │
└─────────────────────────────────────────────────────────────┘
```

## Bench harness

Each phase reports against `tests/perf/`. Synthetic dataset generator:

```bash
cargo run --release --bin kaizen-bench -- gen \
  --sessions 100000 --events-per-session 100 --workspaces 10
cargo run --release --bin kaizen-bench -- measure --phase 0
```

Outputs JSON: `cold_start_ms`, `refresh_p50_ms`, `refresh_p99_ms`,
`append_p99_us`, `retro_30d_ms`, `rss_mb`, `binary_size_kb`.

Baseline captured before Phase 0 lands. Each PR posts before/after table.

## Quint coverage

New or revised specs:

- `specs/event-log-hot.qnt` — append, rotate, replay invariants (Phase 4).
- `specs/projector-incremental.qnt` — span state machine (Phase 2).
- `specs/daemon-handshake.qnt` — client/server protocol versions (Phase 3).
- Existing `specs/tui-app.qnt` updated for window-based selection (Phase 1).

## Rollout strategy

- Phase 0-2 land on `main`, no flag needed (pure speedup, same behavior).
- Phase 3 ships behind `KAIZEN_DAEMON=1` (opt-in) for one minor release,
  then default-on with `--no-daemon` escape hatch.
- Phase 4 requires `kaizen migrate v2`. Migration is reversible
  (`kaizen migrate v1`) for one minor release window.
- Phase 5 ships independently, additive (`kaizen sessions search`).

## Non-goals

- Distributed / cluster mode — out of scope; localhost-first.
- Real-time multi-user collab — sync remains opt-in batch.
- Replacing SQLite outright — it remains the warm tier (great for
  metadata, transactions, joins on small data).
- Custom storage engine from scratch — reuse rkyv/redb/duckdb/arrow.
