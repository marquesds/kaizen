# Session outcomes

Opt-in **post-stop** snapshots: after a `Stop` hook, kaizen can spawn a detached `kaizen outcomes measure` that runs your test (and optional lint) command in the session workspace and stores one row in `session_outcomes`.

## Enable

In `~/.kaizen/projects/<slug>/config.toml` or `~/.kaizen/config.toml`:

```toml
[collect.outcomes]
enabled = true
test_cmd = "cargo test --quiet"
timeout_secs = 600
# optional:
# lint_cmd = "cargo clippy -- -D warnings"
```

Merge rules: [config.md](config.md) (`[collect.outcomes]`).

These commands are explicit, opt-in subprocesses that run in the workspace.
Their normal tooling may create ignored build artifacts such as `target/`.
Kaizen's own files and state still remain outside the repository.

## Lifecycle

1. Ingest appends the `Stop` event and returns quickly.
2. A child process runs `outcomes measure --workspace <path> --session <id>`.
3. The child opens the workspace database under
   `$KAIZEN_HOME/projects/<slug>/kaizen.db`, loads the session, runs commands
   under `session.workspace`, then stores the outcome.

Invariants are modeled in [`specs/session-outcome.qnt`](../specs/session-outcome.qnt) (measurement only after the session is stopped; terminal state).

## CLI

- **`kaizen outcomes show <id> [--workspace]`** — print the JSON row (test counts, `measured_at_ms`, `measure_error` if any).
- **`kaizen outcomes measure`** — internal; used by ingest (hidden in `kaizen help`).

## Gaps (v1)

- **`revert_lines_14d`**, **PR/CI** columns are left `NULL` until git/CI attribution is defined. Heuristic H28 only fires when `revert_lines_14d` is non-null.
- **Build** result is not populated automatically from a generic `test_cmd` run; extend config later if you need explicit `build_cmd`.

## Retro

H27 (test failure rate), H28 (reverts, when present), H29 (lint / failed tests) consume `session_outcomes` for sessions in the retro window. See [retro.md](retro.md).
