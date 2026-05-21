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
| Code graph sidecar | SQLite + GraphQLite (`~/.kaizen/projects/<slug>/codegraph.db`) |
| Code parsers | tree-sitter + language grammars |
| Tests | #[test] / #[tokio::test] |

## Directory Layout

```
kaizen/
├── src/
│   ├── main.rs          # binary entry point
│   ├── collect/         # transcript tail, hooks, parsers
│   ├── bin_kaizen/      # binary-only CLI schema + dispatch
│   ├── core/            # config, shared types
│   ├── mcp/              # stdio MCP server (see `docs/mcp.md`)
│   ├── proxy/            # local LLM API forwarder + `EventSource::Proxy` (`docs/llm-proxy.md`)
│   ├── telemetry/        # optional exporter fan-out
│   ├── metrics/         # repo indexing + smart metric report
│   ├── shell/           # CLI command implementations
│   ├── store/            # SQLite facade, schema, query/write modules
│   ├── sync/            # outbox + HTTP flush
│   ├── ui/              # TUI
│   └── retro/           # heuristic report engine
├── tests/               # integration tests (+ `quint-connect` under `tests/spec/`)
├── specs/               # Quint specs — see docs/quint-coverage.md
├── docs/                # architecture + design docs
├── Cargo.toml           # dependencies, metadata
└── .cursor/             # agent config (rules, skills, hooks)
```

## Notes

- Full dependency set and version pins: [`Cargo.toml`](../Cargo.toml). The table above is a
  map, not a full graph.
- Long-form user documentation in `docs/` is maintained on **GitHub**; the crate published to
  crates.io excludes `docs/` in `Cargo.toml` for a smaller tarball. Files under `assets/` that are
  pulled in with `include_str!` are included in the package.
