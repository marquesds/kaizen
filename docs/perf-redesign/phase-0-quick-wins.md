# Phase 0 — Quick Wins

Pure speedup, no behavior change, no flag. Lands on `main` directly.
Estimated effort: 1 sprint (1 dev). Estimated speedup: 5-10× cold start.

## Scope

### 0.1 SQLite PRAGMAs (`src/store/sqlite.rs:388`)

Replace:

```rust
conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")?;
```

With:

```rust
conn.execute_batch("
    PRAGMA journal_mode=WAL;
    PRAGMA busy_timeout=5000;
    PRAGMA synchronous=NORMAL;
    PRAGMA cache_size=-65536;          -- 64 MB page cache
    PRAGMA mmap_size=268435456;        -- 256 MB mmap
    PRAGMA temp_store=MEMORY;
    PRAGMA wal_autocheckpoint=1000;
")?;
```

Read-only connections (TUI, retro, summary) get the same plus
`PRAGMA query_only=ON;`.

### 0.2 Missing indexes

Add migration:

```sql
CREATE INDEX IF NOT EXISTS tool_spans_session_idx
    ON tool_spans(session_id);
CREATE INDEX IF NOT EXISTS tool_spans_started_idx
    ON tool_spans(started_at_ms);
CREATE INDEX IF NOT EXISTS session_samples_ts_idx
    ON session_samples(ts_ms);
CREATE INDEX IF NOT EXISTS events_ts_idx
    ON events(ts_ms);
CREATE INDEX IF NOT EXISTS feedback_session_idx
    ON session_feedback(session_id);
```

### 0.3 Kill `COALESCE` blind spots

`src/store/sqlite.rs:1743-1780` (`tool_spans_in_window`): replace
`COALESCE(ts.started_at_ms, ts.ended_at_ms, 0) BETWEEN ? AND ?` with
two range queries UNION ALL'd, each hitting its own index. Or
backfill `started_at_ms NOT NULL` and use it directly.

### 0.4 Debounce TUI refresh

`src/ui/tui.rs:603` (tick loop): replace fixed 500ms poll with
event-driven refresh:

- Spawn one `notify::RecommendedWatcher` on `.kaizen/kaizen.db-wal`.
- On WAL change → set `dirty: AtomicBool`.
- Render loop checks `dirty`, calls `refresh()` only if true.
- Coalesce bursts: max one refresh per 100ms.

Idle TUI now consumes ~0% CPU instead of polling SQLite at 2Hz.

### 0.5 Span tree cache

`src/store/span_tree.rs`: cache last computed tree by
`(session_id, last_event_seq)`. Invalidate on append. TUI calls
`session_span_tree()` cheap on hit.

### 0.6 Background `build_report`

`src/ui/tui.rs:135`: move `report::build_report` off the refresh path.
Spawn a `tokio::task` that recomputes report at most once per 2s and
publishes via `Arc<ArcSwap<Option<Report>>>`. TUI reads pointer, never
blocks.

### 0.7 Drop redundant `list_sessions` calls

`refresh()` calls `list_sessions()` every tick — replaced by
notify-driven refresh (0.4). Sessions list is incremental: keep
`max(started_at_ms)` cursor; query only newer rows + status updates
for known IDs.

## Acceptance criteria

Bench harness (`tests/perf/`) with 10k sessions, 1M events:

| Metric | Before | Target |
|---|---|---|
| TUI cold start | <15s | <500ms |
| Refresh p99 | ~600ms | <50ms |
| Idle CPU | 3-8% | <0.1% |
| `summary` runtime | ~3s | <300ms |

All existing tests pass: `cargo test && cargo clippy -- -D warnings &&
cargo fmt --check`.

## Rollback

Pure additive. Revert PRAGMAs and indexes via downgrade migration if
needed. No data shape change.

## Risk

- `synchronous=NORMAL` slightly weaker durability than `FULL` —
  acceptable: WAL still crash-safe to last checkpoint, retro replays
  from events.
- `mmap_size=256MB` may surprise on small VMs — gated by
  `KAIZEN_MMAP_MB` env override.
- Index bloat ~5-10% disk — negligible.

## Out of scope

Schema changes, write-path rewrites, new dependencies. All in later
phases.
