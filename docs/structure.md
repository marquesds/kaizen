# Structure

## Purpose

kaizen — Rust binary crate for AI agent session telemetry.
Collects, stores, and analyzes agent session data.

## Stack

| Layer | Tech |
|---|---|
| Language | Rust (edition 2024) |
| Async runtime | tokio |
| Observability | tracing |
| Tests | #[test] / #[tokio::test] |

## Directory Layout

```
kaizen/
├── src/
│   └── main.rs          # binary entry point
├── tests/               # integration tests
├── docs/                # architecture + design docs
├── Cargo.toml           # dependencies, metadata
└── .cursor/             # agent config (rules, skills, hooks)
```

## Notes

<!-- Add stack details, key crates, and layout as they evolve -->
