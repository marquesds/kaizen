# F4 — Trace Hierarchy (Nested Spans)

## Problem

`tool_spans` is a flat list. There is no way to see that a 50-token `bash` span was called
12 times inside a `write_file` loop, or to compute recursive cost across a chain of sub-calls.
Adding parent-child relationships enables subtree cost analysis and loop detection.

## Data model

```sql
ALTER TABLE tool_spans ADD COLUMN parent_span_id      TEXT REFERENCES tool_spans(span_id);
ALTER TABLE tool_spans ADD COLUMN depth               INTEGER NOT NULL DEFAULT 0;
ALTER TABLE tool_spans ADD COLUMN subtree_cost_usd_e6 INTEGER;
ALTER TABLE tool_spans ADD COLUMN subtree_token_count INTEGER;

CREATE INDEX tool_spans_parent        ON tool_spans(parent_span_id);
CREATE INDEX tool_spans_session_depth ON tool_spans(session_id, depth);
```

`subtree_cost_usd_e6` and `subtree_token_count` are computed recursively after
`rebuild_tool_spans_for_session()` runs.

## Parent detection algorithm

Extend `collect/tool_span_index.rs` with `assign_parents()`, called after the flat span list
is built inside `rebuild_tool_spans_for_session()`:

```rust
fn assign_parents(spans: &mut Vec<SpanBuilder>) {
    // Sort by start_ms ASC, end_ms DESC (outermost spans first).
    // For each span S, find the deepest ancestor A where:
    //   A.start_ms <= S.start_ms && A.end_ms >= S.end_ms
    // Set S.parent_span_id = A.span_id, S.depth = A.depth + 1.
}
```

Containment detection uses `started_at_ms` / `ended_at_ms` already stored on `SpanBuilder`.
No new data collection required.

## New type `SpanNode`

New file `store/span_tree.rs`:

```rust
pub struct SpanNode {
    pub span: ToolSpanView,
    pub children: Vec<SpanNode>,
    pub subtree_cost_usd_e6: i64,
    pub subtree_token_count: u64,
}

pub fn build_tree(spans: Vec<ToolSpanView>) -> Vec<SpanNode>
```

`Store::session_span_tree(session_id) -> Result<Vec<SpanNode>>` returns root nodes only.

## CLI render

```
kaizen sessions tree <session-id> [--depth 3] [--json]
```

Example output:

```
session abc123  $0.042  14 spans
├── write_file  12ms  $0.008
│   ├── bash         3ms  $0.001
│   └── read_file    2ms  $0.000
├── bash        45ms  $0.031  ← subtree cost outlier
│   ├── bash     8ms  $0.009
│   └── bash    37ms  $0.022
└── message      1ms  $0.003
```

Cost outliers are flagged when a subtree cost exceeds 40% of the session total.

## New heuristic H18

File: `retro/heuristics/h18.rs`.

- **Input:** `tool_spans` with `depth` and `subtree_cost_usd_e6` in window.
- **Trigger:** max subtree depth ≥4, or max fan-out ≥8 children on a single span.
- **Bet title:** `"Agent looping inside tool chain — [tool] at depth [d] with [n] children"`.
- **`expected_tokens_saved_per_week`:** `subtree_cost_usd_e6 / 1000 * loop_count`.

`RetroAggregates` gains `SpanTreeStats { max_depth: u32, max_fan_out: u32, deepest_span_id: String }`.

## MCP tool

Add `get_session_span_tree` to `mcp/` — returns `SpanNode` JSON. Useful for agent self-inspection
during a session to detect runaway tool chains.

## Integration points

| Location | Change |
|----------|--------|
| `collect/tool_span_index.rs` | `assign_parents()` after flat span build |
| `store/sqlite.rs` | migration, `session_span_tree()` |
| `store/span_tree.rs` | new file — `SpanNode`, `build_tree()` |
| `tui/` | expandable tree view on session detail pane |
| `retro/types.rs` | `SpanTreeStats` in `RetroAggregates` |

## Effort

~3 dev days (algorithm, new store method, render logic, TUI change).
