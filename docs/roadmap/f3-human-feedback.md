# F3 — Session-Level Human Feedback

## Problem

There is no way to mark a session as "this was a bad run" or "interesting edge case." The retro
engine has no human-judgment signal — only automated metrics. Adding lightweight annotation
closes the loop between developer intuition and heuristic improvement.

## Data model

```sql
CREATE TABLE session_feedback (
    id            TEXT    PRIMARY KEY,
    session_id    TEXT    NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    score         INTEGER CHECK(score BETWEEN 1 AND 5),
    label         TEXT    CHECK(label IN ('good','bad','interesting','bug','regression')),
    note          TEXT,
    created_at_ms INTEGER NOT NULL
);
CREATE INDEX session_feedback_session ON session_feedback(session_id);
CREATE INDEX session_feedback_label   ON session_feedback(label, created_at_ms);
```

`score` and `label` are both optional — either is enough. `note` is free-form text.

## New module `src/feedback/`

```
feedback/
  mod.rs
  store.rs     -- fn upsert_feedback(store, FeedbackRecord) -> Result<()>
               -- fn list_feedback_in_window(store, start_ms, end_ms) -> Vec<FeedbackRecord>
  types.rs     -- FeedbackRecord, FeedbackLabel, FeedbackScore
```

## New heuristic H17

File: `retro/heuristics/h17.rs`.

- **Input:** `feedback` rows in window.
- **Trigger:** ≥2 sessions labeled `bad` or `regression`, or mean score ≤2.5 for ≥5 sessions.
- **Bet title:** `"Human-flagged regressions — [n] sessions scored ≤2 in window"`.
- **`expected_tokens_saved_per_week`:** `n_bad * 800`.

Human-identified failures carry high ROI weight because they represent confirmed pain, not
inferred patterns.

`retro/types.rs` `Inputs` gains `feedback: Vec<FeedbackRecord>`.

## MCP tool

Add `annotate_session` to `mcp/`:

```json
{
  "tool": "annotate_session",
  "input": {
    "session_id": "...",
    "score": 3,
    "label": "interesting",
    "note": "took 3 loops on write_file"
  }
}
```

Allows agents to self-annotate during a session without leaving the terminal workflow.

## CLI

```
kaizen sessions annotate <session-id> \
  --score 1-5 \
  --label good|bad|interesting|bug|regression \
  [--note TEXT]

kaizen feedback list [--workspace PATH] [--label bad] [--since 7d] [--json]
```

## Integration points

| Location | Change |
|----------|--------|
| `store/sqlite.rs` | `upsert_feedback()`, `list_feedback_in_window()`, migration |
| `sessions show` | print feedback rows below session detail |
| `tui/` | colored score badge on session list rows |
| `sync/` | include `session_feedback` in `sync_outbox` batches |
| `telemetry/` | emit feedback events to PostHog/Datadog with score as property |

## Effort

~1.5 dev days. No external dependencies — simplest feature in this roadmap.
