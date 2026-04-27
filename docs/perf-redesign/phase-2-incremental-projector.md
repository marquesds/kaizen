# Phase 2 — Incremental Projector

Kill the O(n²) tool_span rebuild. Apply events as deltas to an
in-memory state machine; persist only on span close.
Estimated effort: 1-2 sprints. Estimated speedup: write throughput
10-50×; append p99 from ~50ms to <5ms.

## Scope

### 2.1 Span state machine

New module `src/store/projector.rs`:

```rust
pub struct Projector {
    open_spans: HashMap<ToolCallId, OpenSpan>,
    file_touch: HashMap<SessionId, FileTouchAccum>,
    skill_use: HashMap<SessionId, SkillUseAccum>,
}

pub enum ProjectorEvent {
    SpanOpened(ToolSpan),         // sink: write to spans table
    SpanClosed(ToolSpan),         // sink: update spans + emit metrics
    FileTouched { session, path }, // sink: upsert files_touched
    SkillUsed { session, skill },
}

impl Projector {
    pub fn apply(&mut self, evt: &Event) -> Vec<ProjectorEvent>;
}
```

Pure function. Property-tested against current `rebuild_tool_spans`
output (`tests/spec/projector_parity.rs`).

### 2.2 Replace `rebuild_tool_spans_for_session`

`src/store/sqlite.rs:564`: replace cascading rebuild call with
projector apply. New write path:

```
append_event_with_sync(evt):
  1. INSERT events                   (unchanged)
  2. projector.apply(evt) -> deltas
  3. for each delta: small targeted UPSERT/INSERT
  4. enqueue sync (unchanged)
```

No more "load all events, delete spans, reinsert". Bounded work
per event.

### 2.3 Crash recovery

Projector is in-memory; on startup, rebuild from last-N events of
each *running* session (status != Done). Done sessions are frozen,
skip rebuild.

```rust
pub fn warm_projector(store: &Store) -> Projector {
    let running = store.list_sessions_by_status(SessionStatus::Running);
    let mut p = Projector::default();
    for s in running {
        for e in store.list_events_for_session(&s.id) {
            p.apply(&e);  // deltas discarded; state restored
        }
    }
    p
}
```

Bounded by # running sessions × events-per-session. Typical: <1k
events total at startup.

### 2.4 File/skill/rule accumulators

Today `index_event_derived()` (sqlite.rs:563) does small inserts per
event. Keep, but route through projector so the in-memory accumulator
matches DB state. Cheap dedup before INSERT.

### 2.5 Metrics emission

Each `ProjectorEvent::SpanClosed` emits a `metrics::Sample` (lead
time, tokens, paths). Subscribed by:

- `tracing` for live debug.
- Future Phase 4: cold-tier writer (Parquet append).
- Future Phase 5: tantivy doc append.

## Acceptance criteria

| Metric | After P0+P1 | Target P2 |
|---|---|---|
| Append event p99 (500-evt session) | ~50ms | <5ms |
| Sustained ingest | ~2k evt/s | >50k evt/s |
| `kaizen proxy` overhead | ~5ms/req | <500µs/req |
| Tool span output diff vs old | n/a | byte-identical |

Parity test: replay 1000 real sessions through both old and new
write path, diff `tool_spans` table → must be empty.

Quint spec: `specs/projector-incremental.qnt` models open/close/orphan
state machine; refines into `tool_spans.status in {done, orphaned}`.

## Rollback

Feature flag `KAIZEN_PROJECTOR=legacy` keeps old rebuild path. Flip
default to `incremental` after one minor release with no parity bugs.

## Risk

- Subtle state machine bugs (orphan detection, retry semantics).
  Mitigated by parity test + Quint spec.
- In-memory state lost on daemon crash → recovered from event log
  (2.3). Bounded.
- Memory grows with # open spans. Cap with TTL: spans open > 1h
  marked orphaned and flushed.

## Out of scope

Daemon process split (Phase 3). Storage tiering (Phase 4).

## Dependencies

Phase 0 only. Independent of Phase 1, can land in parallel.
