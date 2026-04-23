---
name: shipping-and-launch
description: >
  Pre-launch checklists and deployment for Rust binaries. Use when preparing to
  deploy to production, building releases, configuring Railway, or managing
  feature flag lifecycles and rollback procedures.
---

# Shipping and Launch

Ship small, ship often. Smaller deploys safer, easier to debug.

## Pre-Launch Checklist

### Code Quality

- [ ] `cargo test` passes
- [ ] `cargo build --release` clean
- [ ] `cargo clippy -- -D warnings` clean
- [ ] `cargo fmt --check` passes
- [ ] No TODO/FIXME blocking launch
- [ ] Code reviewed (five-axis review complete)

### Security

- [ ] No secrets in source code
- [ ] All secrets via env vars
- [ ] `cargo audit` clean
- [ ] `cargo deny check` passes
- [ ] No `unsafe` without justification

### Infrastructure

- [ ] Health check endpoint exists
- [ ] Logging configured (`RUST_LOG` levels)
- [ ] Error reporting configured
- [ ] All required env vars documented

## Release Build

```bash
cargo build --release
./target/release/kaizen
```

## Railway Deployment

Key env vars:

```
RUST_LOG=info
PORT=8080
DATABASE_URL=...  # if applicable
```

Binary starts, listens on `$PORT`.
Health check: `GET /health` returning 200.

## Staged Rollouts

1. Build release binary
2. Deploy to staging
3. Run smoke tests against staging
4. Deploy to production
5. Monitor logs for 15 minutes
6. Issues → rollback immediately, investigate after

## Rollback Procedure

```bash
# Deploy breaks production:
1. Revert to previous deploy (Railway: redeploy previous build)
2. Investigate issue
3. Fix, test, re-deploy
```

**Rule**: revert first, investigate second. Don't debug in production.

## Feature Flag Lifecycle

```rust
fn is_feature_enabled() -> bool {
    std::env::var("ENABLE_FEATURE").as_deref() == Ok("true")
}
```

```
1. Add flag (default: off)
2. Deploy with flag off
3. Enable for testing
4. Enable for all users
5. Remove flag + dead code (clean up!)
```

Don't let flags accumulate. Schedule removal when enabling.

## Monitoring

After launch, watch:

- Error rates in logs
- Response times
- Memory usage (`RUST_LOG=debug` for verbose)
- Crash/panic reports

## Rationalizations vs Reality

| Rationalization | Reality |
|---|---|
| "Monitor later" | Can't see problems = can't fix them |
| "Works in dev" | Prod has different data, load, config |
| "We can hotfix" | Hotfixes under pressure cause more bugs |

## Red Flags

- Deploying without full test suite
- No rollback plan
- Secrets hardcoded instead of env vars
- No health check endpoint
- `cargo audit` failures ignored
- Feature flags accumulating without cleanup
