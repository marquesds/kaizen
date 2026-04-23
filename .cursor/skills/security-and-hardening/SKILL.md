---
name: security-and-hardening
description: >
  Rust security checklist. Use when adding auth, accepting untrusted input,
  or shipping a user-facing endpoint. Covers cargo audit, unsafe, secrets,
  input validation.
---

# Security and Hardening

Prevent vulnerabilities at design time. Don't bolt security on after.

## Input Validation

### Never Unwrap Untrusted Input

```rust
// GOOD: handle parse error explicitly
let id = raw_id.parse::<u64>().map_err(|_| AppError::BadRequest("invalid id"))?;

// BAD: panics on bad input
let id = raw_id.parse::<u64>().unwrap();
```

### Validate at HTTP Boundary

All external input untrusted. Validate in HTTP handler before passing to core logic.

```rust
pub async fn handle_ingest(
    Json(payload): Json<IngestPayload>,
) -> Result<Json<Response>, AppError> {
    let event = payload.validate()?; // reject bad input at boundary
    let result = collector.ingest(event).await?;
    Ok(Json(result))
}
```

## Unsafe Code

```rust
// GOOD: unsafe only when necessary, documented
/// # Safety
/// `ptr` must be valid and aligned for `T`. Caller ensures lifetime.
unsafe fn read_raw<T>(ptr: *const T) -> T { ... }

// BAD: unsafe without justification
unsafe fn do_thing() {
    // no explanation why unsafe needed
}
```

- No `unsafe` without documented justification
- Prefer safe abstractions over raw pointer manipulation
- `cargo clippy` flags many unsafe misuses

## Dependency Security

```bash
# Audit dependencies for known CVEs
cargo audit

# Policy enforcement (licenses, banned crates, allowed sources)
cargo deny check
```

Add both to CI pipeline. Run before every release.

## Secrets Management

```rust
// GOOD: read from env at startup
let api_key = std::env::var("API_KEY")
    .expect("API_KEY must be set");

// BAD: hardcoded secret
const API_KEY: &str = "sk-1234abcd";
```

- Never commit secrets to version control
- Use `.env` files locally (gitignored), env vars in prod
- Log sanitization: never log secrets or tokens

## Denial of Service

```rust
// BAD: unbounded read from untrusted source
let mut buf = Vec::new();
reader.read_to_end(&mut buf).await?; // attacker can send infinite bytes

// GOOD: bounded read
let mut buf = vec![0u8; MAX_PAYLOAD_SIZE];
let n = reader.read(&mut buf).await?;
```

## Input Validation Checklist

- [ ] All external input validated before core logic
- [ ] No `unwrap()` on values from untrusted sources
- [ ] No `unsafe` without documented justification
- [ ] Secrets via env vars, not hardcoded
- [ ] Payload size limits on HTTP endpoints
- [ ] `cargo audit` in CI pipeline
- [ ] Dependencies reviewed in `cargo deny check`

## Common Rationalizations

| Rationalization | Reality |
|---|---|
| "It's internal tool" | Internal tools get compromised. Same standards |
| "Rust is memory-safe so it's secure" | Memory safety ≠ application security |
| "We'll add auth later" | Auth is architecture. Retrofitting is 10x harder |

## Red Flags

- `unwrap()` on values from HTTP/CLI input
- `unsafe` without documented safety invariants
- Secrets in source code or logs
- No `cargo audit` in CI
- Unbounded reads from external sources
- `String::from_utf8_unchecked` on untrusted bytes
