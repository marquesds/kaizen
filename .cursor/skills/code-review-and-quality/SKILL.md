---
name: code-review-and-quality
description: >
  Multi-axis code review before merge. Use before merging any change, after
  completing a feature, or when evaluating code written by another agent.
  Covers correctness, readability, architecture, security, and performance.
---

# Code Review and Quality

Every change reviewed before merge. Five axes, no exceptions.

## Five-Axis Review

### 1. Correctness

- Matches spec/task requirements?
- Edge cases handled (None, empty, boundary values)?
- Error paths handled (not just happy path)?
- Tests cover change? Testing right things?

### 2. Readability and Simplicity

- Names descriptive and consistent with project conventions?
- Control flow straightforward (no deep nesting)?
- Fewer lines / simpler patterns possible?
- Iterator chains clear and readable?
- Abstractions earning their complexity?

### 3. Architecture

- Follows existing module patterns?
- Clean module boundaries (no storage calls in handlers)?
- Dependencies flow correctly (no circular deps)?
- Appropriate abstraction level?

### 4. Security

- User input validated at boundaries (HTTP handlers)?
- No `unwrap()` on untrusted input?
- No secrets in code or logs?
- Auth checked where needed?
- No `unsafe` without justification?

### 5. Performance

- Unnecessary clones? (check clone abuse)
- Blocking calls in async context?
- Unbounded allocations?
- Missing `#[inline]` on hot paths?

## Change Sizing

```
~100 lines  → Good. Reviewable in one sitting
~300 lines  → Acceptable if single logical change
~1000 lines → Too large. Split it
```

## Severity Labels

| Prefix | Meaning | Action |
|--------|---------|--------|
| *(none)* | Required | Must fix before merge |
| **Critical:** | Blocks merge | Security, data loss, broken functionality |
| **Nit:** | Minor | Author may ignore |
| **Optional:** | Suggestion | Worth considering |
| **FYI** | Info only | No action needed |

## Review Process

1. **Context**: understand what change does and why
2. **Tests first**: read tests to understand intent and coverage
3. **Implementation**: walk through code with five axes
4. **Categorize**: label findings with severity
5. **Verify**: tests pass, clippy clean, fmt clean

## Rust-Specific Checks

```rust
// GOOD: match over nested if-let
fn classify(event: &Event) -> Category {
    match event.kind {
        EventKind::Start => Category::Lifecycle,
        EventKind::Tool(_) => Category::Action,
        EventKind::End => Category::Lifecycle,
    }
}

// BAD: deep if-let nesting
fn classify(event: &Event) -> Category {
    if let EventKind::Start = event.kind {
        Category::Lifecycle
    } else if let EventKind::Tool(_) = event.kind {
        Category::Action
    } else { Category::Lifecycle }
}

// GOOD: iterator chain
let active: Vec<_> = sessions
    .iter()
    .filter(|s| s.is_active())
    .map(summarize)
    .collect();

// BAD: clone abuse
let all_sessions = sessions.clone(); // cloning large vec unnecessarily
```

## Common Rationalizations

| Rationalization | Reality |
|---|---|
| "It works, good enough" | Working + unreadable = debt that compounds |
| "I wrote it, it's correct" | Authors blind to own assumptions |
| "We'll clean up later" | Later never comes. Review IS quality gate |
| "Tests pass, so it's good" | Tests necessary but not sufficient |

## Red Flags

- PRs merged without review
- `unwrap()` on Result/Option from external input
- `unsafe` blocks without documented justification
- No regression tests with bug fixes
- Large PRs "too big to review" (split them)
