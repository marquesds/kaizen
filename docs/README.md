# docs/

Index of kaizen documentation.

| Doc | Purpose |
|---|---|
| [structure.md](structure.md) | Purpose, stack, directory layout |
| [architecture.md](architecture.md) | Module graph, data flow, external boundaries |
| [datamodel.md](datamodel.md) | Data structs, relationships, invariants |
| [config.md](config.md) | Crate roles, config, env vars |
| [patterns.md](patterns.md) | Design patterns, conventions, unusual choices |
| [impl-sequence.md](impl-sequence.md) | Build order M0–M7, done-signals per feature |
| [roadmap.md](roadmap.md) | Milestone table, risks, v0.1 definition of done |
| [ingest-contract.md](ingest-contract.md) | HTTP ingest API (sync daemon → server) |
| [retro.md](retro.md) | Heuristic retro engine (M5) |
| [experiments.md](experiments.md) | Experiments v0 (M6) |

## Keep Docs Current

| Change type | Update |
|---|---|
| New dependency | `config.md` |
| New/changed data struct | `datamodel.md` |
| New module | `architecture.md` |
| New env var | `config.md` |
| New pattern or decision | `patterns.md` |
| Directory layout change | `structure.md` |
| Milestone / scope / risks | `roadmap.md` |
| Ingest or sync server contract | `ingest-contract.md` |
| Retro or experiments product spec | `retro.md` / `experiments.md` |

Stale docs worse than no docs — keep current or delete.
