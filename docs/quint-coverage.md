# Quint spec coverage

Formal models live in [`specs/`](../specs/). CI runs `scripts/check-quint-specs.sh` (typecheck every `*.qnt`). Behavior tests that replay specs use [`quint-connect`](https://crates.io/crates/quint-connect) under [`tests/spec/`](../tests/spec/).

When to add or extend a spec: see [`.cursor/rules/quint-before-code.mdc`](../.cursor/rules/quint-before-code.mdc) (state machines, invariants, concurrent lifecycles). Skip for trivial or read-only changes.

## Specs and tests (paired)

| Spec | `quint-connect` test | CLI / area |
|------|----------------------|------------|
| [`specs/ingest-idempotency.qnt`](../specs/ingest-idempotency.qnt) | `tests/spec/ingest_idempotency.rs` | ingest |
| [`specs/hook-ingest.qnt`](../specs/hook-ingest.qnt) | `tests/spec/hook_ingest.rs` | ingest hook |
| [`specs/session-lifecycle.qnt`](../specs/session-lifecycle.qnt) | `tests/spec/session_lifecycle.rs` | session states (abstract) |
| [`specs/init-setup.qnt`](../specs/init-setup.qnt) | `tests/spec/init_setup.rs` | `init` (global wiring, legacy-local detection) |
| [`specs/project-lookup.qnt`](../specs/project-lookup.qnt) | `tests/spec/project_lookup.rs` | `--project` / `kaizen projects list` resolution |
| [`specs/retention.qnt`](../specs/retention.qnt) | `tests/spec/retention.rs` | tier aging |
| [`specs/event-log-hot.qnt`](../specs/event-log-hot.qnt) | unit tests in `src/store/hot_log.rs` | hot log append, replay, index |
| [`specs/sync-backpressure.qnt`](../specs/sync-backpressure.qnt) | `tests/spec/sync_backpressure.rs` | sync |
| [`specs/daemon-handshake.qnt`](../specs/daemon-handshake.qnt) | `tests/spec/daemon_lifecycle.rs` | daemon lifecycle, background start readiness, protocol retry |
| [`specs/telemetry-exporters.qnt`](../specs/telemetry-exporters.qnt) | `tests/spec/telemetry_exporters.rs` | telemetry (query authority + N-way fan-out + Datadog log records carry `timestamp`, `hostname`, `project_name`, and span metrics for event/tool_span canonical kinds) |
| [`specs/telemetry-file-metadata.qnt`](../specs/telemetry-file-metadata.qnt) | `tests/spec/telemetry_file_metadata.rs` | file NDJSON: envelope vs body-only metadata, tail follow, missing-file tail behavior |
| [`specs/telemetry-push-replay.qnt`](../specs/telemetry-push-replay.qnt) | `tests/spec/telemetry_push_replay.rs` | `telemetry push` (exporter replay, chunking) |
| [`specs/provider-pull-cache.qnt`](../specs/provider-pull-cache.qnt) | `tests/spec/provider_pull_cache.rs` | provider pull / remote cache |
| [`specs/workspace-facts-sync.qnt`](../specs/workspace-facts-sync.qnt) | `tests/spec/workspace_facts_sync.rs` | outbox kind order: `events` before `workspace_facts` |
| [`specs/provider-pull-event-keys.qnt`](../specs/provider-pull-event-keys.qnt) | `tests/spec/provider_pull_event_keys.rs` | pull upsert dedupes `(session_id_hash, event_seq)` composite keys |
| [`specs/mcp-server.qnt`](../specs/mcp-server.qnt) | `tests/spec/mcp_server.rs` | MCP |
| [`specs/llm-proxy.qnt`](../specs/llm-proxy.qnt) | `tests/spec/llm_proxy.rs` | proxy |
| [`specs/experiment-lifecycle.qnt`](../specs/experiment-lifecycle.qnt) | `tests/spec/experiment_lifecycle.rs` | experiment lifecycle (Draft→Running→Concluded→Archived) |
| [`specs/experiment-binding.qnt`](../specs/experiment-binding.qnt) | `tests/spec/experiment_binding.rs` | classification resolution: manual precedence, prompt fingerprint exact match, reclassification safety |
| [`specs/experiment-sequential.qnt`](../specs/experiment-sequential.qnt) | `tests/spec/experiment_sequential.rs` | sequential testing: Significant-is-sticky monotonicity invariant |
| [`specs/auto-update.qnt`](../specs/auto-update.qnt) | `tests/spec/auto_update.rs` | auto-update |
| [`specs/redaction-completeness.qnt`](../specs/redaction-completeness.qnt) | `tests/spec/redaction_completeness.rs` | redaction (forbidden content vs allowlisted labels) |
| [`specs/retro-pipeline.qnt`](../specs/retro-pipeline.qnt) | `tests/spec/retro_pipeline.rs` | retro pipeline (source + optional `RemotePull`) |
| [`specs/doctor-diagnostic.qnt`](../specs/doctor-diagnostic.qnt) | `tests/spec/doctor_diagnostic.rs` | `doctor` checks |
| [`specs/gc-prune.qnt`](../specs/gc-prune.qnt) | `tests/spec/gc_prune.rs` | `gc` / vacuum ordering |
| [`specs/observe-pipeline.qnt`](../specs/observe-pipeline.qnt) | `tests/spec/observe_pipeline.rs` | `sessions list`, `summary`, `insights`, `guidance` (source; mixed dedupe in spec) |
| [`specs/summary-cost-rollup.qnt`](../specs/summary-cost-rollup.qnt) | `tests/spec/summary_cost_rollup.rs` | `summary` / MCP zero–cost rollup footnote rule |
| [`specs/session-lookup.qnt`](../specs/session-lookup.qnt) | `tests/spec/session_lookup.rs` | `sessions show` |
| [`specs/session-tree.qnt`](../specs/session-tree.qnt) | `tests/spec/session_tree.rs` | `sessions tree` empty/missing/render mode contract |
| [`specs/metrics-pipeline.qnt`](../specs/metrics-pipeline.qnt) | `tests/spec/metrics_pipeline.rs` | `metrics` git and plain-workspace index paths |
| [`specs/tui-app.qnt`](../specs/tui-app.qnt) | `tests/spec/tui_app.rs` | TUI lifecycle plus virtualized window invariants |
| [`specs/eval-h15.qnt`](../specs/eval-h15.qnt) | `tests/spec/eval_h15.rs` | H15 eval trigger invariants |
| [`specs/h33-automation.qnt`](../specs/h33-automation.qnt) | `tests/spec/h33_automation.rs` | H33 run / subseq gates and token scalars |
| [`specs/openclaw-ingest.qnt`](../specs/openclaw-ingest.qnt) | `tests/spec/openclaw_ingest_spec.rs` | OpenClaw workspace filter (accept/reject) |
| [`specs/prompt-tracking.qnt`](../specs/prompt-tracking.qnt) | `tests/spec/prompt_tracking_spec.rs` | prompt snapshot lifecycle (SessionStart capture, Stop re-capture, prompt_changed event) |
| [`specs/session-feedback.qnt`](../specs/session-feedback.qnt) | `tests/spec/session_feedback_spec.rs` | H17 human feedback trigger (bad/regression count, mean score threshold) |
| [`specs/guidance-candidate.qnt`](../specs/guidance-candidate.qnt) | `tests/spec/guidance_candidate.rs` | Guidance candidate lifecycle and apply safety (backup, one artifact, prompt-bound experiment) |
| [`specs/guidance-proposal-llm.qnt`](../specs/guidance-proposal-llm.qnt) | `tests/spec/guidance_proposal_llm.rs` | Optional LLM proposal gate (explicit flag, enabled config, redaction, max ops, rejected memory) |
| [`specs/guidance-score.qnt`](../specs/guidance-score.qnt) | `tests/spec/guidance_score.rs` | Guidance score validation gate (`no_data`, `needs_more_validation`, `stable`, `regression`) |
| [`specs/span-hierarchy.qnt`](../specs/span-hierarchy.qnt) | `tests/spec/span_hierarchy_spec.rs` | `assign_parents` invariants: containment, depth consistency, no cycles, root depth=0 |
| [`specs/projector-incremental.qnt`](../specs/projector-incremental.qnt) | `tests/spec/projector_incremental.rs` | Incremental projector: open spans memory-only; persisted rows terminal (`done` or `orphaned`) |
| [`specs/llm-call-quality.qnt`](../specs/llm-call-quality.qnt) | `tests/spec/llm_call_quality.rs` | Phase 1 — per-request retry FSM: retry_count monotonic, event on terminal outcome |
| [`specs/agent-behavior.qnt`](../specs/agent-behavior.qnt) | `tests/spec/agent_behavior.rs` | Phase 2 — mode transitions, todo lifecycle (created monotonic, completed+cancelled bounded), interrupts |
| [`specs/session-outcome.qnt`](../specs/session-outcome.qnt) | `tests/spec/session_outcome.rs` | Phase 4 — outcome side-state: measurement never before Stop; Measured is terminal |
| [`specs/system-sampler.qnt`](../specs/system-sampler.qnt) | `tests/spec/system_sampler.rs` | Phase 5 — sampler lifecycle: Off→Tracking→Stopped; samples only while active; pid valid when tracking |
| [`specs/search.qnt`](../specs/search.qnt) | `tests/spec/search.rs` | Phase 5 — search lifecycle: append/commit/reindex/delete/fallback parity |
| [`specs/upgrade.qnt`](../specs/upgrade.qnt) | `tests/spec/upgrade.rs` | `upgrade` — install-method detection → single upgrade action; no-execute-without-method invariant |

**Hook / init models:** [`hook-ingest.qnt`](../specs/hook-ingest.qnt) treats `codex`, `copilot-cli`, and `openclaw` as known hook sources alongside `cursor` and `claude`. [`init-setup.qnt`](../specs/init-setup.qnt) includes five hook-host slots (added `openclaw`); models user-global wiring and `legacy_cursor_local`/`legacy_claude_local` detection invariants; runtime `kaizen init` patches Cursor, Claude Code, and OpenClaw. [`project-lookup.qnt`](../specs/project-lookup.qnt) models `resolve_project_name` — three outcome states (Found, NotFound, Ambiguous) with bijective invariants.

## Integration tests without `quint-connect`

- [`tests/spec/doctor_cmd.rs`](../tests/spec/doctor_cmd.rs) — smoke `doctor` output in a temp workspace.
- [`tests/spec/guidance_cmd.rs`](../tests/spec/guidance_cmd.rs) — smoke `guidance` JSON in an empty workspace (pipeline also modeled by `observe-pipeline.qnt`).
- [`tests/spec/guidance_score_validation.rs`](../tests/spec/guidance_score_validation.rs) — seeded CLI JSON proof for held-out validation regression (gate modeled by `guidance-score.qnt`).

## Surfaces intentionally without a Quint module

- **`completions`** — static codegen; low value for formal modeling.

**Ingest** and **sync** are covered by hook / ingest idempotency and sync-backpressure specs. **Retro** has `retro-pipeline` for pipeline phases; heuristic-specific gates include **`h33-automation`** (H33). Not every heuristic in [`src/retro/`](../src/retro/) has a Quint module. **`retention.qnt`** models tier **aging**, not every `gc` edge case; **`gc-prune.qnt`** captures refuse / prune / vacuum ordering.

## When to close a gap

Add or extend `specs/*.qnt` when you introduce **multi-step protocols**, **ordering / idempotency**, or **concurrent lifecycles** that are easy to get wrong. Keep smoke tests for end-to-end output where a formal model would add little.
