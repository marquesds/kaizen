---
name: debugging-and-error-recovery
description: >
  Systematic root-cause debugging for Rust. Use when tests fail, builds break,
  behavior is unexpected, or any error appears. Provides structured triage
  process instead of guessing.
---

# Debugging and Error Recovery

Something breaks: stop, preserve evidence, follow triage checklist.

## Stop-the-Line Rule

```
1. STOP adding features or changes
2. PRESERVE evidence (error output, logs, repro steps)
3. DIAGNOSE via triage checklist
4. FIX root cause
5. GUARD with regression test
6. RESUME after verification passes
```

## Triage Checklist

### Step 1: Reproduce

```bash
cargo test                          # full suite
cargo test test_name                # specific test
cargo test -- --nocapture           # see println! output
RUST_BACKTRACE=1 cargo test         # full backtraces
RUST_BACKTRACE=full cargo test      # maximum detail
```

### Step 2: Localize

```
Which layer is failing?
├── Compile error      → Check types, lifetimes, imports, trait bounds
├── Borrow checker     → Check ownership, references, lifetimes
├── Panic at runtime   → Check unwrap/expect/index, enable RUST_BACKTRACE=1
├── Logic error        → Check match arms, iterator chains, off-by-one
├── Async/await        → Check executor, Send bounds, blocking in async
└── Test itself        → Check if test is correct (false negative)
```

### Step 3: Reduce

Strip failing case to minimum that reproduces bug.

### Step 4: Fix Root Cause

```rust
// Symptom fix (BAD): silence with unwrap_or
let val = risky_call().unwrap_or_default();

// Root cause fix (GOOD): handle error explicitly
let val = risky_call().map_err(|e| {
    tracing::error!("risky_call failed: {e}");
    AppError::Internal
})?;
```

Ask "why does this happen?" until reaching actual cause.

### Step 5: Guard with Test

```rust
#[test]
fn handles_special_characters_in_event_content() {
    let content = r#"Fix "quotes" & <brackets>"#;
    let event = Event::new(content);
    assert_eq!(event.content(), content);
}
```

### Step 6: Verify End-to-End

```bash
cargo test
cargo clippy -- -D warnings
cargo fmt --check
```

## Error Patterns

| Error | Check |
|-------|-------|
| `unwrap()` panic | Add proper error handling; use `?` operator |
| Borrow checker error | Check lifetime annotations, ownership transfer |
| `?` propagation fail | Ensure error types implement `From<SourceError>` |
| Trait not satisfied | Check bounds, ensure type implements required traits |
| Async `Send` error | Avoid holding non-Send types across `.await` |
| Stack overflow | Check recursive functions, add iterative alternative |

## RUST_BACKTRACE

```bash
RUST_BACKTRACE=1 cargo run          # concise backtrace
RUST_BACKTRACE=full cargo run       # full frames
RUST_LOG=debug cargo run            # verbose tracing output
```

## Common Rationalizations

| Rationalization | Reality |
|---|---|
| "I know what the bug is" | Right 70% of time. Reproduce first |
| "Failing test is wrong" | Verify. If wrong, fix it. Don't skip |
| "Works locally" | Check CI, features, cfg flags |
| "I'll fix next commit" | Fix now. Next commit builds on broken state |

## Red Flags

- Skipping failing test to work on new features
- Guessing fixes without reproducing
- Using `unwrap()` to silence errors temporarily
- No regression test after bug fix
- `#[allow(unused)]` to suppress compiler warnings
