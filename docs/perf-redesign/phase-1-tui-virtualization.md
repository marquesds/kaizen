# Phase 1 — TUI Virtualization

Render only the viewport. Lazy-fetch on scroll. Async data provider.
Estimated effort: 1 sprint. Estimated speedup: cold start <500ms even
on 100k sessions (UI no longer blocks on full session list).

## Scope

### 1.1 Paged `list_sessions`

Add to `Store`:

```rust
pub fn list_sessions_page(
    &self,
    workspace: &str,
    offset: usize,
    limit: usize,
    filter: SessionFilter,  // agent, status, since_ms
) -> Result<SessionPage>;

pub struct SessionPage {
    pub rows: Vec<SessionRecord>,
    pub total: usize,         // cached COUNT, refreshed lazily
    pub next_offset: Option<usize>,
}
```

Index: `sessions_workspace_started_idx (workspace, started_at_ms DESC)`.
Existing `list_sessions()` becomes a wrapper for back-compat.

### 1.2 Virtualized list state

`src/ui/tui.rs` replaces `sessions_all: Vec<SessionRecord>` with:

```rust
struct SessionView {
    page_size: usize,         // = viewport height + 2*prefetch
    cursor: usize,            // selected index (logical)
    window: BTreeMap<usize, SessionRecord>,  // sparse cache
    total: usize,
}
```

Render reads `window[cursor..cursor+viewport]`. Scroll past edge →
async fetch next page; LRU evict far pages.

### 1.3 Lazy event/span load

Selecting a session triggers fetch of *only its* events + spans, not
preloaded for all. Already partially true; formalize as
`SessionDetail` future:

```rust
enum DetailState {
    Idle,
    Loading(JoinHandle<DetailData>),
    Ready(DetailData),
}
```

TUI shows "loading…" spinner if select happens during fetch.

### 1.4 Event pagination inside session

Sessions with 10k+ events crash the detail pane today. Add:

```rust
pub fn list_events_page(
    &self, session_id: &str,
    after_seq: u64, limit: usize,
) -> Result<Vec<Event>>;
```

Detail pane keeps a virtualized event list, same pattern as 1.2.

### 1.5 Filter pushed to SQL

Today `reapply_filter()` (tui.rs:101) filters in Rust over `sessions_all`.
Push to SQL `WHERE agent LIKE ?` with parameterized prefix. Avoids
loading filtered-out rows.

### 1.6 Feedback batched on demand

`feedback_for_sessions()` called for *all visible* IDs every refresh
(tui.rs:136-143). Restrict to viewport IDs only, cache by session_id.

## Acceptance criteria

| Metric | After P0 | Target P1 |
|---|---|---|
| TUI cold start (100k sess) | ~3s | <300ms |
| Memory at startup | ~500 MB | <50 MB |
| Scroll latency | n/a (all loaded) | <16ms |
| Filter response | ~200ms | <50ms |

`tests/spec/tui_app.qnt` extended: window invariants (cursor always
in fetched range, no double-load).

## Rollback

Direct mode (no daemon yet) → revert to Phase 0. New paged APIs are
additive; old `list_sessions()` kept.

## Risk

- TUI state machine more complex (loading states). Mitigated by
  `DetailState` enum + property test (`tests/spec/tui_window.rs`).
- SQL `LIKE 'agent%'` may scan if no index — add expression index
  on `lower(agent)`.

## Out of scope

Daemon. Snapshot push. Search. All later.

## Dependencies

Requires Phase 0 (PRAGMAs + indexes) for paged queries to be cheap.
