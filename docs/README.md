# docs/

Index.

## For users

| Doc | Purpose |
|---|---|
| [install.md](install.md) | Install, build from source, uninstall |
| [usage.md](usage.md) | CLI reference |
| [llm-proxy.md](llm-proxy.md) | Local HTTP forwarder for LLM APIs (Anthropic) |
| [concepts.md](concepts.md) | Sessions, events, retro, experiments |
| [config.md](config.md) | Config file + env vars |
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
| New pattern or decision | `patterns.md` or new ADR |
| Directory layout change | `structure.md` |
| Ingest / sync contract | `ingest-contract.md` |
| Retro / experiments product spec | `retro.md` / `experiments.md` |
| New CLI command or flag | `usage.md` |

Stale docs worse than no docs — keep current or delete.
