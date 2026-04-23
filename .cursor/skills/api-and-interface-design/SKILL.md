---
name: api-and-interface-design
description: >
  Guides stable API and module interface design. Use when designing Rust
  traits, module boundaries, public functions, or any surface where one
  module talks to another. Covers contract-first design and boundary validation.
---

# API and Interface Design

Design stable, hard-to-misuse interfaces.

## Core Principles

### Hyrum's Law

Every observable behavior becomes de facto contract. Be intentional about what's exposed. Don't leak implementation details.

### Contract First

Define module API before implementing:

```rust
pub trait SessionStore {
    /// Store session. Returns Err if session_id already exists.
    fn create(&self, session: Session) -> Result<(), StoreError>;

    /// Fetch session. Returns None if not found.
    fn get(&self, id: SessionId) -> Option<Session>;

    /// List sessions for agent, newest first.
    fn list_for_agent(&self, agent_id: AgentId) -> Vec<Session>;

    /// Delete session. Idempotent — succeeds if already deleted.
    fn delete(&self, id: SessionId) -> Result<(), StoreError>;
}
```

### Consistent Error Semantics

Pick one pattern, use everywhere:

```rust
Ok(result)              // success
Err(AppError::NotFound) // resource not found
Err(AppError::Invalid)  // validation failure
Err(AppError::Denied)   // permission denied
```

Don't mix patterns. Consumers can't predict behavior if some functions return `Ok(_)` and others panic.

### Validate at Boundaries

Trust internal code. Validate where external input enters:

```rust
pub async fn handle_ingest(
    State(store): State<Arc<dyn SessionStore>>,
    Json(payload): Json<IngestPayload>,
) -> Result<Json<IngestResponse>, AppError> {
    let event = payload.validate()?;
    let result = store.ingest(event).await?;
    Ok(Json(result))
}
```

Validate at: HTTP handlers, CLI argument parsing, external service responses, env var loading.
Not between internal functions sharing type contracts.

### Prefer Addition Over Modification

```rust
// GOOD: add optional parameter via builder
pub fn list_sessions(agent_id: AgentId) -> SessionQuery { ... }
// Later: SessionQuery::new(agent_id).limit(50).since(ts)

// BAD: change signature — breaks all callers
// fn list_sessions(agent_id: AgentId) → fn list_sessions(agent_id: AgentId, limit: usize)
```

## Rust Module Patterns

### Deep Modules with Honest Interfaces

```rust
// GOOD: simple interface hiding query building, filtering, auth
pub fn list_sessions_for_agent(agent_id: AgentId, opts: ListOpts) -> Vec<Session>;

// BAD: leaking implementation details
pub fn list_sessions_for_agent(
    agent_id: AgentId,
    query_builder: QueryBuilder,
    filter_list: Vec<Filter>,
    auth_check: fn(&Session) -> bool,
) -> Vec<Session>;
```

### Result<T, E> Error Semantics

Use typed errors. No `unwrap` on untrusted input. `?` for propagation.

```rust
// GOOD
pub fn parse_session_id(raw: &str) -> Result<SessionId, ParseError> {
    raw.parse().map_err(ParseError::InvalidFormat)
}

// BAD
pub fn parse_session_id(raw: &str) -> SessionId {
    raw.parse().unwrap() // panics on bad input
}
```

## Common Rationalizations

| Rationalization | Reality |
|---|---|
| "We'll document later" | `///` doc comments ARE docs. Define first |
| "Internal APIs don't need contracts" | Internal consumers are still consumers |
| "We can just change it" | Hyrum's Law — someone depends on current behavior |

## Red Flags

- Functions returning different shapes depending on conditions
- Inconsistent error patterns across module
- Validation scattered inside internal functions
- Breaking changes to public function signatures
- `unwrap()` on values from external input
- Leaking internal types across module boundaries
