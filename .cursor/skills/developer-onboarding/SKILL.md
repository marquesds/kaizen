---
name: developer-onboarding
description: >
  Onboards new developers to kaizen. Use when user says "I'm new", "how do
  I start", "first time setup", "help me onboard", "where do I begin", asks
  for project tour, or needs Rust orientation. Covers stack, local setup,
  codebase map, design decisions, first contribution.
---

# Developer Onboarding

Goal: newcomer runs binary + holds mental model in ~30 minutes. Prefer linking existing docs over duplicating.

## 1. Triage First

Ask 3 questions, branch rest on answers:

1. Rust experience — none / some / fluent?
2. Today's goal — just build it / add telemetry / architecture tour?
3. Env state — fresh clone / partial setup / stuck on a step?

Skip sections that don't match. Don't dump full walkthrough.

## 2. What kaizen Is

Rust binary crate for agent session telemetry. Collects, stores, and analyzes AI agent session data.
See `docs/structure.md` and `docs/architecture.md`.

## 3. Stack Snapshot

| Layer | Tech | Notes |
|---|---|---|
| Language | Rust (edition 2024) | systems lang |
| Async runtime | tokio | async I/O |
| HTTP | axum or hyper | TBD |
| Storage | TBD | see docs/datamodel.md |
| Observability | tracing | structured logs |
| Tests | #[test] / #[tokio::test] | built-in + tokio |

Full list: `docs/structure.md`.

## 4. Rust Primer (skip if fluent)

Pointers, not tutorial:

- Rust book — https://doc.rust-lang.org/book/
- Async Rust — https://rust-lang.github.io/async-book/
- tokio — https://tokio.rs/tokio/tutorial
- Edition in `Cargo.toml`

Mental model:

- **Crate** (`src/`) = unit of compilation. Binary crate has `main.rs`.
- **Modules** (`mod`) = namespaced code. `pub` items form public API.
- **Traits** = shared behavior contracts (like interfaces).
- **Result<T, E>** = explicit error handling. `?` propagates errors.
- **Ownership + lifetimes** = memory safety without GC.

## 5. Local Setup Checklist

1. `rustup update stable` — ensure Rust stable toolchain
2. `cargo build` — download deps, compile
3. `cargo test` — run test suite
4. `cargo run` — run binary

Verify:

```bash
cargo test && cargo clippy -- -D warnings && cargo fmt --check
```

## 6. Codebase Map

Doc index: `docs/README.md`. Layout: `docs/structure.md`.

First files to open:

- `src/main.rs` — binary entry point
- `Cargo.toml` — dependencies, metadata
- `docs/architecture.md` — module graph, data flow

## 7. Design Decisions

- Core principles in `AGENTS.md` §8:
  - Functional Core / Imperative Shell — pure `fn` for logic, `async fn` at I/O boundary
  - Sinks, Not Pipes — components consume input, do work, stop
  - AI-Ready Architecture — module boundaries discoverable without reading internals
  - Simplicity First — minimal impact per change, no temp workarounds

## 8. Conventions to Internalize

From `AGENTS.md` + `.cursor/rules/`:

- Caveman writing in all `.md`/`.mdc`, doc comments, inline comments, plans
- ≤200 lines per file (incl. `.md`)
- ≤10 lines per function, no cyclomatic branching — iterator chains + match
- No narrating comments
- `docs/` stays current with structural change
- `cargo test && cargo clippy -- -D warnings && cargo fmt --check` before "done"
- Plan mode for any 3+ step task

Full set: `.cursor/rules/*.mdc`.

## 9. First Contribution Flow

Use sibling skills:

- `tdd` — failing test first, then implement
- `incremental-implementation` — thin vertical slices
- `git-workflow-and-versioning` — atomic commits, trunk-based
- `code-review-and-quality` — multi-axis review before merge
- `project-exploration` — docs-first when touching unfamiliar area

## 10. Common Stumbles

| Symptom | Fix |
|---|---|
| Borrow checker errors | Read error message fully; check ownership transfer |
| `async` trait issues | Use `async_trait` crate or associated types |
| Test not running | Check test name filter: `cargo test my_test` |
| Clippy warnings as errors | Fix lint before committing |
| Compilation slow | Use `cargo check` for fast type-checking without codegen |

## 11. Smoke Test

```bash
cargo build
cargo test
cargo run
```

All three green → env works.

## 12. Anti-Patterns

| Anti-Pattern | Fix |
|---|---|
| Skip docs, grep source | Read `docs/structure.md` first |
| `unwrap()` on user input | Handle errors with `?` and typed errors |
| Skip `cargo clippy` | Run before declaring done |
| Invent module structure | Match existing patterns |
| Ignore compiler warnings | Fix them; warnings are bugs waiting to happen |
