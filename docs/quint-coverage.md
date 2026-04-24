# Quint spec coverage

Formal models live in [`specs/`](../specs/). CI runs `scripts/check-quint-specs.sh` (typecheck every `*.qnt`). Behavior tests that replay specs use [`quint-connect`](https://crates.io/crates/quint-connect) under [`tests/spec/`](../tests/spec/).

When to add or extend a spec: see [`.cursor/rules/quint-before-code.mdc`](../.cursor/rules/quint-before-code.mdc) (state machines, invariants, concurrent lifecycles). Skip for trivial or read-only changes.

## Specs and tests (paired)

| Spec | `quint-connect` test | CLI / area |
|------|----------------------|------------|
| [`specs/ingest-idempotency.qnt`](../specs/ingest-idempotency.qnt) | `tests/spec/ingest_idempotency.rs` | ingest |
| [`specs/hook-ingest.qnt`](../specs/hook-ingest.qnt) | `tests/spec/hook_ingest.rs` | ingest hook |
| [`specs/session-lifecycle.qnt`](../specs/session-lifecycle.qnt) | `tests/spec/session_lifecycle.rs` | session states (abstract) |
| [`specs/init-setup.qnt`](../specs/init-setup.qnt) | `tests/spec/init_setup.rs` | `init` |
| [`specs/retention.qnt`](../specs/retention.qnt) | `tests/spec/retention.rs` | tier aging |
| [`specs/sync-backpressure.qnt`](../specs/sync-backpressure.qnt) | `tests/spec/sync_backpressure.rs` | sync |
| [`specs/telemetry-exporters.qnt`](../specs/telemetry-exporters.qnt) | `tests/spec/telemetry_exporters.rs` | telemetry |
| [`specs/mcp-server.qnt`](../specs/mcp-server.qnt) | `tests/spec/mcp_server.rs` | MCP |
| [`specs/llm-proxy.qnt`](../specs/llm-proxy.qnt) | `tests/spec/llm_proxy.rs` | proxy |
| [`specs/experiment-lifecycle.qnt`](../specs/experiment-lifecycle.qnt) | `tests/spec/experiment_lifecycle.rs` | experiments |
| [`specs/auto-update.qnt`](../specs/auto-update.qnt) | `tests/spec/auto_update.rs` | auto-update |
| [`specs/redaction-completeness.qnt`](../specs/redaction-completeness.qnt) | `tests/spec/redaction_completeness.rs` | redaction |
| [`specs/retro-pipeline.qnt`](../specs/retro-pipeline.qnt) | `tests/spec/retro_pipeline.rs` | retro pipeline |
| [`specs/doctor-diagnostic.qnt`](../specs/doctor-diagnostic.qnt) | `tests/spec/doctor_diagnostic.rs` | `doctor` checks |
| [`specs/gc-prune.qnt`](../specs/gc-prune.qnt) | `tests/spec/gc_prune.rs` | `gc` / vacuum ordering |
| [`specs/observe-pipeline.qnt`](../specs/observe-pipeline.qnt) | `tests/spec/observe_pipeline.rs` | `sessions list`, `summary`, `insights`, `guidance` (shared read path) |
| [`specs/session-lookup.qnt`](../specs/session-lookup.qnt) | `tests/spec/session_lookup.rs` | `sessions show` |
| [`specs/metrics-pipeline.qnt`](../specs/metrics-pipeline.qnt) | `tests/spec/metrics_pipeline.rs` | `metrics` |
| [`specs/tui-app.qnt`](../specs/tui-app.qnt) | `tests/spec/tui_app.rs` | TUI lifecycle |

**Hook / init models:** [`hook-ingest.qnt`](../specs/hook-ingest.qnt) treats `codex` and `copilot-cli` as known hook sources alongside `cursor` and `claude`. [`init-setup.qnt`](../specs/init-setup.qnt) includes four hook-host slots on the same pattern; runtime `kaizen init` may still only patch Cursor and Claude until extended.

## Integration tests without `quint-connect`

- [`tests/spec/doctor_cmd.rs`](../tests/spec/doctor_cmd.rs) — smoke `doctor` output in a temp workspace.
- [`tests/spec/guidance_cmd.rs`](../tests/spec/guidance_cmd.rs) — smoke `guidance` JSON in an empty workspace (pipeline also modeled by `observe-pipeline.qnt`).

## Surfaces intentionally without a Quint module

- **`completions`** — static codegen; low value for formal modeling.

**Ingest** and **sync** are covered by hook / ingest idempotency and sync-backpressure specs. **Retro** has `retro-pipeline` for pipeline phases, not every heuristic in [`src/retro/`](../src/retro/). **`retention.qnt`** models tier **aging**, not every `gc` edge case; **`gc-prune.qnt`** captures refuse / prune / vacuum ordering.

## When to close a gap

Add or extend `specs/*.qnt` when you introduce **multi-step protocols**, **ordering / idempotency**, or **concurrent lifecycles** that are easy to get wrong. Keep smoke tests for end-to-end output where a formal model would add little.
