# Experiments — Hypothesis Testing v0

Test codebase changes that should make agents better. Bind a window of
sessions to control vs treatment, compute metric delta with bootstrap CI,
write markdown report. No client-side feature flags, no production
splits — just attribution over an existing event stream.

## Use Cases

- "Adding `.cursor/skills/rust-tdd/` reduces tokens/session by 10%."
- "Switching architecture to hexagonal cuts edit-loop count per session."
- "Tighter `read-hygiene` rule cuts cost without dropping success rate."
- "Renaming module X reduces cross-file co-edits."

## Data Model

```rust
pub struct Experiment {
    pub id: String,
    pub name: String,
    pub hypothesis: String,
    pub change_description: String,
    pub metric: Metric,
    pub binding: Binding,
    pub duration_days: u32,
    pub success_criterion: Criterion,
    pub state: State,        // Draft | Running | Concluded | Archived
    pub created_at: i64,
    pub concluded_at: Option<i64>,
}

pub enum Metric {
    TokensPerSession,
    CostPerSession,
    SuccessRate,            // sessions ending without error
    ToolLoops,              // repeated edit→test_fail→edit count
    DurationMinutes,
    FilesPerSession,
}

pub enum Binding {
    GitCommit  { control_commit: String, treatment_commit: String },
    Branch     { control_branch: String, treatment_branch: String },
    ManualTag  { variant_field: String }, // event payload key holding 'A'|'B'
}

pub enum Criterion {
    Delta { direction: Direction, target_pct: f64 }, // e.g. -10% TokensPerSession
    Absolute { metric_value: f64 },
}
```

## Hybrid Binding Resolution

Default: `GitCommit`. Auto-detected at session start by walking
`git log` between session timestamps. Manual override always wins.

Resolution order, per session:

1. If session has `experiment_tag` event in its stream (set by
   `kaizen exp tag`), use `ManualTag`.
2. Else if session's `start_commit` is ancestor of `treatment_commit`
   and descendant of `control_commit`, classify as treatment.
3. Else if ancestor of `control_commit` only, classify as control.
4. Else `Excluded` (e.g. session straddled the boundary commit).

Branch binding is a special case of GitCommit where control/treatment
commits = tip of each branch at evaluation time.

## Statistics

Non-parametric to survive small N and skewed distributions.

- **Effect size**: median(treatment) − median(control). Mean reported
  alongside but not primary.
- **CI**: bootstrap, 10k resamples, 95% percentile interval on the
  median delta.
- **Sample-size warning**: if `min(N_control, N_treatment) < 30`,
  attach a warning band and skip rendering CI as "significant".
- **Outlier policy**: winsorize at p1 / p99 by default (configurable
  in `[experiments]`).

No p-values. CI excludes zero in the wrong direction → success.

## CLI

```bash
kaizen exp new \
  --name add-rust-tdd-skill \
  --hypothesis "rust-tdd skill cuts tokens/session by 10%" \
  --change "added .cursor/skills/rust-tdd/" \
  --metric tokens_per_session \
  --bind git \
  --duration 14d \
  --target -10%

kaizen exp list
kaizen exp status <id>
kaizen exp tag <id> --variant treatment   # manual override
kaizen exp report <id>                    # markdown + bootstrap CI
kaizen exp conclude <id>
```

## Worked Example 1 — Add a Skill

Day 0: write skill, commit. Run:

```
kaizen exp new --name add-rust-tdd --metric tokens_per_session \
  --bind git --duration 14d --target -10%
```

Kaizen records `control_commit = HEAD~1`, `treatment_commit = HEAD`.
For 14 days, every new session is auto-classified at session-end based
on `start_commit` ancestry. Day 14: `kaizen exp report <id>` emits:

```
# Experiment: add-rust-tdd

State: Concluded · Window: 2026-04-22 → 2026-05-06
Hypothesis: rust-tdd skill cuts tokens/session by 10%
Binding: git (control 8b2a1c, treatment d4f9e0)

Metric: tokens_per_session

         N    median    mean
control  41   18,420    21,103
treatment 38  15,902    17,488

Delta (median): −2,518 tokens (−13.7%)
95% bootstrap CI on delta: [−4,102, −1,030]
Target: −10%   → MET

Caveats:
- N per arm > 30, CI considered reliable.
- 3 sessions excluded (straddled boundary commit).
```

## Worked Example 2 — Architecture Refactor

Hexagonal refactor lands across 18 files in one commit. Hypothesis:
agents loop less because module boundaries are clearer.

```
kaizen exp new --name hex-refactor --metric tool_loops \
  --bind git --duration 21d --target -25%
```

Same flow. Branch binding works too if the refactor ships behind a
feature branch with parallel work on `main`.

## Quint Spec

`/specs/experiment-lifecycle.qnt` — covers two state machines:

```
states: Draft → Running → Concluded → Archived
allowed transitions:
  Draft → Running        (via exp start)
  Running → Concluded    (via duration elapsed OR exp conclude)
  Concluded → Archived   (via exp archive)
invariants:
  - Running experiment has well-formed binding (commits exist in repo)
  - Concluded experiment has frozen binding + frozen sample
  - No event ever reclassifies after Concluded
  - Manual tag wins over git binding when both present
  - GitCommit binding total: every session is Treatment | Control | Excluded
```

`quint-connect` test in `tests/spec/experiment_lifecycle.rs` replays
traces against the Rust impl.

## Future (v0.2+)

- Multi-arm experiments (A / B / C / D).
- Composite metrics (`tokens_per_session × success_rate`).
- Auto-conclude when CI excludes zero in target direction (early stop).
- Counterfactual replays — rerun past sessions with new skill set,
  needs Tier 3 proxy capture for fidelity.
- Experiment-bound retro: "what bets did this experiment generate?"
