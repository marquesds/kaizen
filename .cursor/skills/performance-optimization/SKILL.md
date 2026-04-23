---
name: performance-optimization
description: >
  Measure-first Rust perf tuning. Use ONLY when user reports a perf
  regression or asks to optimize a specific function/async task/query.
---

# Performance Optimization

Measure first. Optimize second. Never optimize without evidence.

## Measure-First Rule

```
1. IDENTIFY symptom (slow path, high memory, timeout)
2. MEASURE with profiling tools (not guessing)
3. LOCATE bottleneck (hot function? allocation? I/O?)
4. FIX specific bottleneck
5. VERIFY improvement with same measurement
```

## Rust Profiling Tools

### Criterion (Microbenchmarks)

```rust
use criterion::{criterion_group, criterion_main, Criterion};

fn bench_ingest(c: &mut Criterion) {
    let event = Event::new_test();
    c.bench_function("ingest_event", |b| {
        b.iter(|| collector.ingest(event.clone()))
    });
}

criterion_group!(benches, bench_ingest);
criterion_main!(benches);
```

```bash
cargo bench
```

### Flamegraph

```bash
cargo install flamegraph
cargo flamegraph --bin kaizen
# opens flamegraph.svg
```

### tokio-console (Async Task Profiling)

```bash
cargo install tokio-console
# Add to Cargo.toml: tokio-console-subscriber
# Run binary with TOKIO_CONSOLE_BIND=127.0.0.1:6669
tokio-console
```

### perf + samply

```bash
# Linux
perf record --call-graph dwarf ./target/release/kaizen
perf report

# macOS (samply)
cargo install samply
samply record ./target/release/kaizen
```

## Common Bottlenecks

| Symptom | Likely Cause | Fix |
|---------|-------------|-----|
| High CPU | Hot loop doing unnecessary work | Profile with flamegraph |
| High memory | Excessive cloning, leaking allocations | Use `Arc`, `Cow`, refs |
| Slow async | Blocking call in async context | Move to `spawn_blocking` |
| Timeout | Unbounded loop or I/O | Add timeout, pagination |
| Throughput wall | Single-threaded bottleneck | Parallelize with rayon/tokio |
| Allocation pressure | Vec reallocation in hot path | Pre-allocate with `with_capacity` |

## Allocation Optimization

```rust
// BAD: repeated reallocation
let mut buf = Vec::new();
for item in &items { buf.push(process(item)); }

// GOOD: pre-allocate
let mut buf = Vec::with_capacity(items.len());
for item in &items { buf.push(process(item)); }

// BETTER: iterator chain (zero intermediate alloc)
let buf: Vec<_> = items.iter().map(process).collect();
```

## Async Optimization

```rust
// BAD: sequential awaits
let a = fetch_a().await?;
let b = fetch_b().await?;

// GOOD: concurrent awaits
let (a, b) = tokio::try_join!(fetch_a(), fetch_b())?;
```

## Verification

- [ ] Improvement measured with same tool/methodology
- [ ] No functional regressions (`cargo test` passes)
- [ ] Optimization documented (why, what was measured)
- [ ] Evidence justified change (no premature optimization)

## Common Rationalizations

| Rationalization | Reality |
|---|---|
| "It feels slow" | Measure it. Feelings aren't benchmarks |
| "This should be faster" | Profile it. Bottleneck is rarely where you think |
| "Optimize everything" | Optimize bottleneck. 80/20 rule applies |

## Red Flags

- Optimizing without profiling first
- Blocking calls (`std::thread::sleep`, `std::fs`) in async context
- Unnecessary clones in hot paths
- Missing `with_capacity` on known-size collections
- Premature optimization of non-bottleneck code
