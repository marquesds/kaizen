# F1 — LLM-as-a-Judge Evaluations

## Problem

Retro heuristics H1–H14 are entirely count- and threshold-based. H9 (errors) fires when
`EventKind::Error` count reaches 6. H10 fires when shell exit-code-1 results reach 3 per session.
No heuristic asks *why* an agent struggled or whether a particular tool sequence was sensible.
LLM-as-a-Judge fills that gap by scoring session quality qualitatively.

## Data model

New table `session_evals`:

```sql
CREATE TABLE session_evals (
    id            TEXT    PRIMARY KEY,
    session_id    TEXT    NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    judge_model   TEXT    NOT NULL,
    rubric_id     TEXT    NOT NULL,
    score         REAL    NOT NULL CHECK(score BETWEEN 0.0 AND 1.0),
    rationale     TEXT    NOT NULL,
    flagged       INTEGER NOT NULL DEFAULT 0,
    created_at_ms INTEGER NOT NULL
);
CREATE INDEX session_evals_session ON session_evals(session_id);
CREATE INDEX session_evals_rubric  ON session_evals(rubric_id, score);
```

`rubric_id` is a slug like `"tool-efficiency-v1"`. `rationale` is redacted before sync when
`[privacy] redact_payloads = true`. `flagged = 1` means the eval is surfaced in the retro report.

## New module `src/eval/`

```
eval/
  mod.rs       -- pub use
  rubric.rs    -- Rubric { id, name, prompt_template, score_range }
  judge.rs     -- fn judge_session(client, rubric, session, events) -> EvalResult
  engine.rs    -- fn run_evals(store, cfg, workspace, since_ms) -> Vec<EvalResult>
  types.rs     -- EvalResult, EvalConfig
```

`judge.rs` flow:

1. Build compressed session summary from `Vec<Event>`: tool sequence, error events,
   `cost_usd_e6` total, duration.
2. Interpolate `rubric.prompt_template` with summary.
3. POST to `[eval] endpoint` (re-uses the proxy HTTP client).
4. Parse JSON response `{score: f32, rationale: str}`.
5. Store via `Store::upsert_eval()`.

## Config section

```toml
[eval]
enabled      = true
endpoint     = "https://api.anthropic.com"
model        = "claude-haiku-4-5-20251001"
rubric       = "tool-efficiency-v1"
batch_size   = 20
min_cost_usd = 0.01
```

`min_cost_usd` skips trivial sessions to avoid spending tokens on one-line edits.

## New heuristic H15

File: `retro/heuristics/h15.rs`.

- **Input:** `session_evals` rows in window joined with `sessions`.
- **Trigger:** ≥3 sessions with `score < 0.4`, or mean score dropped >15% vs prior window.
- **Bet title:** `"Low eval scores — agent struggling with [rubric_id]"`.
- **`expected_tokens_saved_per_week`:** `n_low_sessions * 600`.

`retro/types.rs` `Inputs` gains `eval_scores: Vec<(String, f64)>` (session_id, score).

## CLI

```
kaizen eval run  [--workspace PATH] [--since 7d] [--dry-run]
kaizen eval list [--workspace PATH] [--min-score 0.4] [--json]
```

## Integration points

| Location | Change |
|----------|--------|
| `store/sqlite.rs` | `upsert_eval()`, `list_evals_in_window()`, migration |
| `retro/engine.rs` | pass `eval_scores` into `Inputs`, call `h15::bets()` |
| `sessions show` | append eval score + rationale block |
| `sync/` | add `session_evals` to `sync_outbox` batch kinds |

## Effort

~3 dev days.
