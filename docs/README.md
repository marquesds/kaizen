# docs/

Index.

## Start here (learning)

If you are new to **what** gets stored and **in what order**, read
[telemetry-journey.md](telemetry-journey.md) first. It connects transcripts, hooks, and the
optional HTTP proxy to the SQLite tables and derived facts. Use [concepts.md](concepts.md) as a
glossary.

## For users

| Doc | Purpose |
|---|---|
| [install.md](install.md) | Install, build from source, uninstall |
| [telemetry-journey.md](telemetry-journey.md) | How agent sessions map to data (ingest → events → facts) |
| [usage.md](usage.md) | CLI reference |
| [mcp.md](mcp.md) | MCP stdio server (agent hosts, full CLI parity) |
| [llm-proxy.md](llm-proxy.md) | Local HTTP forwarder for LLM APIs (Anthropic) |
| [concepts.md](concepts.md) | Sessions, events, retro, experiments |
| [config.md](config.md) | Config file + env vars, sync, proxy, pluggable telemetry |
| [retro.md](retro.md) | Heuristic retro engine |
| [retro-tuning.md](retro-tuning.md) | Tuning heuristic thresholds |
| [experiments.md](experiments.md) | Experiments v0 |

## For contributors

| Doc | Purpose |
|---|---|
| [structure.md](structure.md) | Purpose, stack, directory layout |
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
| User-facing learning story or pipeline change | [telemetry-journey.md](telemetry-journey.md), [architecture.md](architecture.md) |

Stale docs worse than no docs — keep current or delete.
