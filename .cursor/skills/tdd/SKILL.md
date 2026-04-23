---
name: tdd
description: >
  Test-driven development for Rust. Use when implementing any logic, fixing
  any bug, or changing any behavior. Every agent must write a failing test
  first, then implement. Use when you need to prove code works.
---

# TDD

Write failing test before code. Tests are proof.

## Cycle

```
RED              GREEN            REFACTOR
Write test   →   Make it pass  →  Clean up  →  (repeat)
that fails       minimal code     tests still pass
```

### RED — Write Failing Test

```rust
// Fails because create_session doesn't exist yet
#[test]
fn creates_session_with_defaults() {
    let store = MemoryStore::new();
    let session = store.create(SessionAttrs {
        agent_id: "agent-1".into(),
        ..Default::default()
    }).unwrap();
    assert_eq!(session.status, Status::Active);
    assert_eq!(session.agent_id.as_str(), "agent-1");
}
```

### GREEN — Make It Pass

```rust
pub fn create(&self, attrs: SessionAttrs) -> Result<Session, StoreError> {
    let session = Session {
        id: SessionId::new(),
        agent_id: attrs.agent_id,
        status: Status::Active,
        created_at: Utc::now(),
    };
    self.sessions.write().insert(session.id, session.clone());
    Ok(session)
}
```

### REFACTOR — Clean Up

Tests green → improve code without changing behavior. Run `cargo test` after every step.

## Prove-It Pattern (Bug Fixes)

Don't fix first. Reproduce with test.

```
Bug report → Write reproduction test → Test FAILS (bug confirmed)
→ Implement fix → Test PASSES → Run full suite (no regressions)
```

```rust
// Bug: "completing a session doesn't set completed_at"
#[test]
fn sets_completed_at_when_session_completes() {
    let store = MemoryStore::new();
    let session = store.create(default_attrs()).unwrap();
    let completed = store.complete(session.id).unwrap();
    assert_eq!(completed.status, Status::Completed);
    assert!(completed.completed_at.is_some()); // fails → bug confirmed
}
```

## Test Pyramid

```
       /\        Integration (~15%)
      /  \       Full HTTP round-trip, real storage
     /----\      Unit (~85%)
    /      \     Pure functions, structs, no I/O
```

## Good Tests

- **State not interactions**: assert outcomes, not mock calls
- **DAMP over DRY**: each test tells complete story
- **Real implementations**: avoid mocks for pure logic
- **One assertion per concept**: separate test per behavior
- **Descriptive names**: reads like spec

```rust
// GOOD
mod complete_session {
    #[test]
    fn sets_status_to_completed() { ... }

    #[test]
    fn returns_error_for_already_completed() { ... }

    #[test]
    fn returns_error_for_unknown_session() { ... }
}

// BAD
#[test]
fn session_works() { ... }

#[test]
fn handles_errors() { ... }
```

## Rust Test Patterns

```rust
// Async test
#[tokio::test]
async fn broadcasts_event_on_ingest() {
    let collector = Collector::new_test();
    collector.ingest(test_event()).await.unwrap();
    // assert on effect
}

// Test with setup
fn test_store() -> MemoryStore {
    MemoryStore::new()
}

// Integration test in tests/
// tests/integration_test.rs
```

## Compact Verify (in-loop runs)

```bash
cargo test test_name              # single test by name
cargo test -- --nocapture         # see println! output during test
cargo test 2>&1 | tail -5         # just the summary line
```

Full commands:

```bash
cargo test                        # full suite
cargo test --test integration     # integration tests only
cargo test -- --test-threads=1    # serial (for shared state)
```

## Rationalizations vs Reality

| Rationalization | Reality |
|---|---|
| "Write tests after" | Won't. Tests after fact test implementation, not behavior |
| "Too simple to test" | Simple gets complicated. Test documents expected behavior |
| "Tests slow me down" | Slow now, fast every future change |
| "Tested manually" | Manual doesn't persist. Next change might break it |

## Red Flags

- Code without tests
- Tests passing on first run (may not test what you think)
- Bug fixes without reproduction tests
- `#[allow(dead_code)]` or `#[allow(unused)]` to pass tests
- No `#[tokio::test]` on async test functions
