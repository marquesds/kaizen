# F2 — Prompt/System-Prompt Version Tracking

## Problem

Sessions capture `start_commit` and `end_commit` (code state) but not which `CLAUDE.md`,
`AGENTS.md`, or `.cursor/rules/*.mdc` were active. The question "did changing the system prompt
improve cost or error rate?" is unanswerable with the current schema.

## Data model

New table `prompt_snapshots`, plus a column on `sessions`:

```sql
CREATE TABLE prompt_snapshots (
    fingerprint    TEXT    PRIMARY KEY,
    captured_at_ms INTEGER NOT NULL,
    files_json     TEXT    NOT NULL,
    total_bytes    INTEGER NOT NULL
);

ALTER TABLE sessions ADD COLUMN prompt_fingerprint TEXT
    REFERENCES prompt_snapshots(fingerprint);
```

`files_json` stores `[{path, sha256, bytes}]` for every scanned prompt file. `fingerprint` is
SHA-256 over sorted file contents.

## New module `src/prompt/`

```
prompt/
  mod.rs
  snapshot.rs    -- fn capture(workspace) -> PromptSnapshot
                 -- fn fingerprint(files: &[PromptFile]) -> String
  diff.rs        -- fn diff(a: &PromptSnapshot, b: &PromptSnapshot) -> PromptDiff
  types.rs       -- PromptSnapshot, PromptFile, PromptDiff
```

`snapshot::capture` scans (relative to workspace root):

- `CLAUDE.md`, `AGENTS.md`, `CONTRIBUTING.md`
- `.cursor/rules/*.mdc`
- `.cursor/skills/*/SKILL.md`

## Hook integration

On `SessionStart`: call `snapshot::capture(workspace)`, store via `Store::upsert_prompt_snapshot()`,
set `session.prompt_fingerprint`.

On `SessionEnd`: re-capture; if fingerprint changed, emit `EventKind::Hook` with payload
`{kind: "prompt_changed", from_fingerprint, to_fingerprint}`.

## New experiment metrics

`experiment/types.rs` gains two new `Metric` variants:

```rust
pub enum Metric {
    // existing …
    SuccessRateByPrompt,
    CostByPrompt,
}
```

These group sessions by `prompt_fingerprint` and compare aggregates across groups, using the
same bootstrap CI as existing metrics.

## New heuristic H16

File: `retro/heuristics/h16.rs`.

- **Input:** sessions in window grouped by `prompt_fingerprint`.
- **Trigger:** ≥2 distinct fingerprints with ≥5 sessions each; mean cost differs >20% or error
  rate differs >15%.
- **Bet title:** `"Prompt [short_hash] underperforms [short_hash] — [metric] diff [delta]%"`.
- **`expected_tokens_saved_per_week`:** `n_sessions_on_worse_fingerprint * 500`.

`retro/types.rs` `Inputs` gains `prompt_fingerprints: Vec<(String, String)>` (session_id, fingerprint).

## CLI

```
kaizen prompt list [--workspace PATH] [--json]
kaizen prompt show <fingerprint>
kaizen prompt diff <fingerprint_a> <fingerprint_b>
kaizen exp new --metric CostByPrompt --binding prompt-version
```

## Integration points

| Location | Change |
|----------|--------|
| `store/sqlite.rs` | `upsert_prompt_snapshot()`, `get_prompt_snapshot()`, migration |
| `collect/hooks/` | capture on `SessionStart` hook event |
| `retro/types.rs` | `prompt_fingerprints` in `Inputs` |
| `sessions show` | print active fingerprint + file list |

## Effort

~2 dev days.
