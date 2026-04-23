# Patterns

## Design Patterns

### Functional Core / Imperative Shell

Pure `fn` for logic. `async fn` only at I/O boundary.
See `AGENTS.md` §8 for rationale.

### Sinks, Not Pipes

Components receive input, do work, stop. No cascading side effects.

### Error Handling

`Result<T, E>` everywhere. `?` for propagation. No `unwrap()` on external input.
Typed error enums per module.

## Conventions

- Max 200 lines per file
- Max 10 lines per function
- Iterator chains over manual loops
- `match` over nested `if let`
- `///` doc comments on all public items

## Unusual Choices

<!-- Document deviations from Rust idioms and why -->
