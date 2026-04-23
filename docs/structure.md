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
| Local store | SQLite WAL |
| Code graph sidecar | SQLite + GraphQLite (`.kaizen/codegraph.db`) |
| Code parsers | tree-sitter + language grammars |
| Tests | #[test] / #[tokio::test] |

## Directory Layout

```
kaizen/
├── src/
│   ├── main.rs          # binary entry point
│   ├── metrics/         # repo indexing + smart metric report
│   ├── sync/            # event/span/snapshot sync
│   └── retro/           # heuristic report engine
├── tests/               # integration tests
├── docs/                # architecture + design docs
├── Cargo.toml           # dependencies, metadata
└── .cursor/             # agent config (rules, skills, hooks)
```

## Notes

<!-- Add stack details, key crates, and layout as they evolve -->
