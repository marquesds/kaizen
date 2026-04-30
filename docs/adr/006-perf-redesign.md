# ADR 006: Performance Redesign — Tiered Storage + Daemon

## Status
Accepted

## Context

TUI cold start + refresh slow at scale (thousands of agents/sessions). Diagnose
in `src/ui/tui.rs:67-145` + `src/store/sqlite.rs`:

- `App::open()` synchronous: `list_sessions()` (all rows) + `ensure_indexed()`
  (FS scan + codegraph) + `build_report(7d)` (file_facts + tool_spans full).
- `refresh()` polls every 500ms, re-runs full scan + per-session events + spans
  + tree + report + feedback. No paging, no debounce.
- Write path O(n²): `rebuild_tool_spans_for_session()` (sqlite.rs:564) reloads
  entire session, deletes spans, reinserts on every event. 500-event session
  → 500 full rebuilds.
- Indexes missing: `tool_spans.session_id`, `tool_spans.started_at_ms`,
  `session_samples.ts_ms`. `COALESCE(...)` in time-window queries blinds index.
- PRAGMAs anemic: only WAL + busy_timeout. No `synchronous=NORMAL`,
  `cache_size`, `mmap_size`, `temp_store=MEMORY`.
- Payload JSON raw in TEXT, re-parsed every read. Span tree rematerialized
  every refresh (no cache).
- `--all-workspaces` opens N SQLite files sequentially.

Spike E (ADR 001) measured SQLite OK at 1k sessions. We are now past 100k
threshold; ADR 001 explicitly defers to ADR 006 here.

## Decision

Phased redesign, no big bang. Five phases, each shippable standalone:

| Phase | Theme | Dep | Risk |
|---|---|---|---|
| 0 | Quick wins (PRAGMAs, indexes, debounce, cache) | none | low |
| 1 | TUI virtualization + lazy fetch | 0 | low |
| 2 | Incremental projector (kill O(n²) rebuild) | 0 | med |
| 3 | Daemon split (single writer + IPC) | 2 | high |
| 4 | Tiered storage (hot mmap + warm SQLite + cold Parquet + DuckDB) | 3 | high |
| 5 | Tantivy search index for prompts | 0 | med |

Hard targets after Phase 4:
- TUI cold start < 100ms with 100k sessions / 10M events.
- Refresh delta < 16ms (60fps).
- Write throughput > 50k events/sec sustained.
- `kaizen retro --days 30 --all-workspaces` < 1s on 1M events.

Stack additions (all single-binary, no servers):
- `duckdb` crate — embedded columnar query engine.
- `redb` — pure-Rust KV for hot index.
- `rkyv` — zero-copy event log.
- `arrow` — IPC between daemon and clients.
- `tantivy` — full-text search index.

## Alternatives

- **Keep SQLite, fix only Phase 0-2**: cheapest, but caps at ~10k sessions
  before list/window queries degrade again. Defers — does not solve.
- **Postgres / ClickHouse**: violates localhost / single-binary constraint.
- **Pure in-memory (no persistence)**: loses crash safety, retro replay.
- **Rewrite TUI in TUI framework X**: bottleneck is data layer, not render.

## Consequences

- New process model: optional `kaizen daemon` (auto-spawn). CLI stays usable
  standalone in "direct mode" (Phase 0-2 wins still apply).
- Schema migration tool: `kaizen migrate v2` reads SQLite → writes hot log +
  Parquet partitions. One-shot, idempotent, reversible.
- Binary size +~20 MB (DuckDB + Arrow + tantivy). Acceptable for the speedup.
- Retention shifts from `DELETE` cascade to `rm` of cold partitions.
- `unsafe` lives in one module (`store::hot_log` rkyv mmap); fuzz-tested.
- Multi-workspace becomes a glob over Parquet files via DuckDB.
