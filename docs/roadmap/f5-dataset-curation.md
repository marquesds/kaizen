# F5 — Dataset Curation from Production Traces

## Problem

Experiments are bound to git commits or branches. There is no way to say "use these 10 specific
sessions as a regression suite" or "benchmark future prompt changes against this curated set of
bad runs." Dataset curation closes the loop between retro findings and structured experimentation.

## Data model

```sql
CREATE TABLE datasets (
    id            TEXT    PRIMARY KEY,
    name          TEXT    NOT NULL UNIQUE,
    description   TEXT,
    created_at_ms INTEGER NOT NULL
);

CREATE TABLE dataset_sessions (
    dataset_id    TEXT    NOT NULL REFERENCES datasets(id)  ON DELETE CASCADE,
    session_id    TEXT    NOT NULL REFERENCES sessions(id)  ON DELETE CASCADE,
    note          TEXT,
    pinned_at_ms  INTEGER NOT NULL,
    PRIMARY KEY (dataset_id, session_id)
);
CREATE INDEX dataset_sessions_dataset ON dataset_sessions(dataset_id);

ALTER TABLE experiments ADD COLUMN dataset_id TEXT REFERENCES datasets(id);
```

## New module `src/dataset/`

```
dataset/
  mod.rs
  engine.rs    -- fn run_against_dataset(store, exp, dataset) -> DatasetReport
  types.rs     -- Dataset, DatasetSession, DatasetReport
```

## New experiment binding variant

`experiment/types.rs`:

```rust
pub enum Binding {
    GitCommit { control_commit: String, treatment_commit: String },
    Branch    { control_branch: String, treatment_branch: String },
    ManualTag { variant_field: String },
    Dataset   { dataset_id: String, variant_field: String },  // new
}
```

`Dataset` binding classifies sessions by `variant_field` from `experiment_tags` — same mechanism
as `ManualTag` but scoped to dataset members only. Experiments can compare metrics across any
two subsets of pinned sessions regardless of git history.

## Feedback integration (F3 → F5)

Extend `sessions annotate` (from F3) with `--pin-to-dataset`:

```
kaizen sessions annotate <session-id> \
  --score 2 --label regression \
  --pin-to-dataset "auth-failures"
```

When `--pin-to-dataset` is given and the dataset does not exist, it is created automatically.

## CLI

```
kaizen dataset new <name> [--description TEXT]
kaizen dataset list [--json]
kaizen dataset show <dataset-id>
kaizen dataset add    <dataset-id> <session-id> [--note TEXT]
kaizen dataset remove <dataset-id> <session-id>

kaizen exp new \
  --metric CostPerSession \
  --binding dataset:<dataset-id> \
  --variant-field label

kaizen exp report <exp-id> --dataset
```

## Integration points

| Location | Change |
|----------|--------|
| `store/sqlite.rs` | `upsert_dataset()`, `add_to_dataset()`, `list_dataset_sessions()`, migration |
| `experiment/binding.rs` | `Dataset` arm in `classify()` |
| `experiment/engine.rs` | filter to dataset sessions when binding is `Dataset` |
| `feedback/` (F3) | `--pin-to-dataset` flag wired to `dataset::add_to_dataset()` |
| `retro/types.rs` | `dataset_session_ids: HashSet<String>` in `Inputs` |

## Why this matters for retros

Once datasets exist, heuristics can check whether a bet-triggering session is already pinned.
A session in a `"known-regressions"` dataset that re-triggers H9 or H17 indicates the bet
was either not acted on or did not resolve the root cause — a signal worth surfacing separately.

## Effort

~2 dev days.
