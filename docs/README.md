# docs/

Index.

## Start here (learning)

If you are new to **what** gets stored and **in what order**, read
[telemetry-journey.md](telemetry-journey.md) first. It connects transcripts, hooks, and the
optional HTTP proxy to the SQLite tables and derived facts. Use [concepts.md](concepts.md) as a
glossary. For a **guided tour** with exercises, start at [tutorial/README.md](tutorial/README.md).

## For users

| Doc | Purpose |
|---|---|
| [install.md](install.md) | `cargo install` from crates.io, build from source, uninstall |
| [telemetry-journey.md](telemetry-journey.md) | How agent sessions map to data (ingest → events → facts) |
| [tutorial/README.md](tutorial/README.md) | Hands-on tutorial (all major features + exercises) |
| [usage.md](usage.md) | CLI reference |
| [mcp.md](mcp.md) | MCP stdio server (agent hosts; most commands as tools) |
| [daemon.md](daemon.md) | Local daemon lifecycle and direct-mode fallback |
| [llm-proxy.md](llm-proxy.md) | Local HTTP forwarder for LLM APIs (Anthropic) |
| [concepts.md](concepts.md) | Sessions, events, retro, experiments |
| [outcomes.md](outcomes.md) | Post-stop test/lint outcomes (opt-in) |
| [system-telemetry.md](system-telemetry.md) | Per-PID CPU/RSS sampling (opt-in) |
| [config.md](config.md) | Config file + env vars, sync, proxy, pluggable telemetry |
| [retro.md](retro.md) | Heuristic retro engine |
| [retro-tuning.md](retro-tuning.md) | Tuning heuristic thresholds |
| [experiments.md](experiments.md) | Experiments v0 |

## For contributors

| Doc | Purpose |
|---|---|
| [structure.md](structure.md) | Purpose, stack, directory layout |
| [quint-coverage.md](quint-coverage.md) | Which features have `specs/*.qnt` vs CLI-only tests |
| [architecture.md](architecture.md) | Module graph, data flow, boundaries |
| [datamodel.md](datamodel.md) | Data structs, relationships, invariants |
| [patterns.md](patterns.md) | Conventions, design patterns |
| [ingest-contract.md](ingest-contract.md) | HTTP ingest API |
| [adr/](adr/) | Architecture decision records |

## Keep docs current

| Change | Update |
|---|---|
| New dependency | `config.md` |
| New/changed data struct | `datamodel.md` |
| New module | `architecture.md` |
| New env var | `config.md` |
| `kaizen proxy` or proxy config | [llm-proxy.md](llm-proxy.md), [config.md](config.md) |
| `kaizen telemetry` or `[[telemetry.exporters]]` | [usage](usage.md), [config](config.md#telemetry) |
| New pattern or decision | `patterns.md` or new ADR |
| Directory layout change | `structure.md` |
| Ingest / sync contract | `ingest-contract.md` |
| Retro / experiments product spec | `retro.md` / `experiments.md` |
| New CLI command or flag | `usage.md` |
| New user-facing feature or meaningful flag / behavior | `usage.md` and the relevant [tutorial](tutorial/README.md) part (or new part) |
| New Quint spec or change to spec/test pairing | [quint-coverage.md](quint-coverage.md) |
| User-facing learning story or pipeline change | [telemetry-journey.md](telemetry-journey.md), [architecture.md](architecture.md), [tutorial](tutorial/README.md) as needed |

Stale docs worse than no docs — keep current or delete.
