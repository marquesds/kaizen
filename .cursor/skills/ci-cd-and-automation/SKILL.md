---
name: ci-cd-and-automation
description: >
  CI/CD pipeline and automation for Rust. Use when setting up or modifying
  build/deploy pipelines, configuring GitHub Actions, or deploying to Railway.
  Covers Shift Left, quality gates, and feature flags.
---

# CI/CD and Automation

Shift Left: catch problems as early as possible in pipeline.

## Quality Gate Pipeline

```
Code Push → Build → Clippy → Format Check → Test → Audit → Deploy
```

### GitHub Actions for Rust

```yaml
name: CI
on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - uses: Swatinem/rust-cache@v2
      - run: cargo build --locked
      - run: cargo clippy -- -D warnings
      - run: cargo fmt --check
      - run: cargo test
      - run: cargo audit
```

### Cargo Precommit Equivalent

Single quality gate:

```bash
cargo test && cargo clippy -- -D warnings && cargo fmt --check
```

Add to `Makefile` as `make precommit` or alias in shell.

## Security Scanning

```bash
# Audit dependencies for known vulnerabilities
cargo audit

# Policy-based dependency checks (licenses, bans, sources)
cargo deny check
```

## Feature Flags

For incomplete features merging incrementally:

```rust
fn is_chat_enabled() -> bool {
    std::env::var("ENABLE_CHAT").as_deref() == Ok("true")
}
```

## Deployment to Railway

- Set `RUST_LOG`, `DATABASE_URL`, `PORT` env vars
- Use `cargo build --release` for production builds
- Health check: `GET /health`
- Binary from `target/release/<binary-name>`

## Shift Left Principles

1. **Compile-time first**: `cargo build --locked`
2. **Lint enforcement**: `cargo clippy -- -D warnings`
3. **Format enforcement**: `cargo fmt --check`
4. **Security**: `cargo audit`, `cargo deny`
5. **Tests**: `cargo test` with `#[tokio::test]` for async

## Faster is Safer

- Small, frequent deploys safer than large, infrequent ones
- Deploy breaks → revert first, investigate second
- Monitoring and alerts catch what tests miss

## Common Rationalizations

| Rationalization | Reality |
|---|---|
| "CI overkill for this project" | CI catches what you forget locally |
| "I'll add CI later" | Later means after broken code is merged |
| "Manual deploy is fine" | Manual deploys are error-prone and slow |

## Red Flags

- No CI pipeline configured
- Tests not running on every push
- No `cargo clippy` in CI
- No `cargo audit` in pipeline
- Deploying without full test suite
