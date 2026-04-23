---
name: code-simplification
description: >
  Simplifies code for clarity without changing behavior. Use when code works but
  is harder to read or maintain than it should be. Use during refactoring, after
  features ship, or when complexity has accumulated.
---

# Code Simplification

Reduce complexity while preserving exact behavior.
Goal: new team member understands it faster than original.

## Principles

### 1. Preserve Behavior Exactly

All inputs, outputs, side effects, error behavior must remain identical.
Unsure simplification preserves behavior → don't make it.

### 2. Follow Project Conventions

Match existing patterns. Simplification breaking consistency is churn.

### 3. Prefer Clarity Over Cleverness

```rust
// UNCLEAR: dense conditional
let status = if is_new { "New" } else if is_updated { "Updated" } else { "Active" };

// CLEAR: match on state
fn status_label(state: State) -> &'static str {
    match state {
        State::New => "New",
        State::Updated => "Updated",
        State::Active => "Active",
    }
}
```

### 4. Chesterton's Fence

Before removing/changing anything, understand why it exists. Check git blame. Can't explain reason → don't touch it.

### 5. Scope to What Changed

Don't refactor unrelated code unless explicitly asked.

## Simplification Patterns

### Deep Nesting → Match

```rust
// BEFORE
fn process(data: Option<Data>) -> Result<Output> {
    if let Some(d) = data {
        if d.is_valid() {
            do_work(d)
        } else {
            Err(AppError::Invalid)
        }
    } else {
        Err(AppError::Missing)
    }
}

// AFTER
fn process(data: Option<Data>) -> Result<Output> {
    let d = data.ok_or(AppError::Missing)?;
    if !d.is_valid() { return Err(AppError::Invalid); }
    do_work(d)
}
```

### Verbose Loop → Iterator Chain

```rust
// BEFORE
let mut results = Vec::new();
for item in &items {
    let s = transform(item);
    results.push(s);
}

// AFTER
let results: Vec<_> = items.iter().map(transform).collect();
```

### Duplicated Logic → Shared Function

```rust
// BEFORE: same formatting in 3 places
format!("{} {}", user.first_name, user.last_name)

// AFTER
fn full_name(user: &User) -> String {
    format!("{} {}", user.first_name, user.last_name)
}
```

### Dead Code → Remove

Unreachable branches, unused functions, commented-out blocks.
Confirm dead before removing. Ask if unsure.

```
DEAD CODE IDENTIFIED:
- format_legacy_date in src/helpers.rs — replaced by format_date
- unused import in src/sessions.rs
→ Safe to remove these?
```

## Process

1. Read code, read tests, check git blame
2. Identify one simplification opportunity
3. Apply change
4. Run `cargo test` — if fails, revert
5. Commit separately from feature work
6. Repeat

## Common Rationalizations

| Rationalization | Reality |
|---|---|
| "It's working, don't touch it" | Hard-to-read code is hard to fix when it breaks |
| "Fewer lines is simpler" | Dense one-liner isn't simpler than clear match |
| "I'll refactor while adding this feature" | Separate refactoring from feature work |

## Red Flags

- Simplification requiring test modifications (behavior changed)
- "Simplified" code longer and harder to follow than original
- Removing error handling for "cleanliness"
- Simplifying code you don't fully understand
- Batching many simplifications in one large commit
