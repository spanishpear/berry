# Benchmarking Guide

Technical documentation for Berry's benchmarking infrastructure.

## Setup

### Prerequisites

```bash
rustup update
cargo build --workspace
cargo test --workspace
```

### Quick Start

```bash
# Quick performance test
cargo run --bin berry-bench-bin -- -f minimal-berry.lock -v

# Run all benchmarks
cargo run --bin berry-bench-bin -- --all -r 5

# Save a baseline for regression checks
cargo run --bin berry-bench-bin -- --all -r 10 --format json --save-baseline .bench/baseline.json

# Compare against baseline with 5% threshold and fail on regression (for CI)
cargo run --bin berry-bench-bin -- --all -r 10 --baseline .bench/baseline.json --threshold-ratio-ms-per-kib 0.05 --fail-on-regression

# Detailed Criterion benchmarks
cargo bench --package berry-bench --bench parser_benchmarks -- --quick
```

## Benchmark Categories

### Fixture Parsing

Tests parsing performance with different file sizes:

```bash
# Small fixtures (1-10 packages)
cargo run --bin berry-bench-bin -- -f minimal-berry.lock
cargo run --bin berry-bench-bin -- -f workspaces.yarn.lock

# Medium fixtures (10-1000 packages)
cargo run --bin berry-bench-bin -- -f auxiliary-packages.yarn.lock

# Large fixtures (1000+ packages)
cargo run --bin berry-bench-bin -- -f berry.lock
cargo run --bin berry-bench-bin -- -f resolutions-patches.yarn.lock
```

**Performance targets**:

- Small files (< 1KB): < 1ms
- Medium files (< 100KB): < 10ms
- Large files (< 1MB): < 100ms

### Memory Usage

Tracks heap usage and validates zero-allocation claims:

```bash
# Memory usage with verbose output
cargo run --bin berry-bench-bin -- -f minimal-berry.lock -v

# Criterion memory benchmarks
cargo bench --package berry-bench --bench parser_benchmarks
```

**Memory targets**:

- Zero allocations during parsing phase
- Minimal heap usage for final data structures
- Consistent memory patterns across similar file sizes

### Input Characteristics

Tests different lockfile features:

```bash
# Test different characteristics
cargo run --bin berry-bench-bin -- -f yarn4-mixed-protocol.lock
cargo run --bin berry-bench-bin -- -f yarn4-resolution.lock
cargo run --bin berry-bench-bin -- -f yarn4-patch.lock
```

## Interpreting Results

### CLI Tool Output

```
Benchmarking minimal-berry.lock (1152 bytes)...
  Warmup 1: 0.165ms
  Warmup 2: 0.132ms
  Warmup 3: 0.134ms
  Heap usage: 20480 bytes (physical), 0 bytes (virtual)
  Run 1: 0.133ms
  Run 2: 0.133ms
  Run 3: 0.131ms

Benchmark Results:
Fixture                   Size (bytes) Mean (ms)    Min (ms)     Max (ms)     Heap (bytes)
------------------------------------------------------------------------------------------
minimal-berry.lock        1152         0.132        0.131        0.133        20480

Performance Analysis:
  minimal-berry.lock performance looks normal (1.0x vs fastest)
```

**Key metrics**:

- **Mean (ms)**: Average parsing time across all runs
- **Min/Max (ms)**: Range of parsing times
- **Heap (bytes)**: Physical memory usage
- **Performance Analysis**: Regression detection results

### Criterion Output

```
fixture_parsing/minimal_berry
                        time:   [6.1249  b5s 6.2624  b5s 6.2968  b5s]
                        change: [-3.4204% -0.9236% +1.4829%] (p = 0.85 > 0.05)
                        No change in performance detected.

heap_usage/heap_small   time:   [1.2025 ms 1.2383 ms 1.2472 ms]
```

**Key metrics**:

- **Time range**: 95% confidence interval
- **Change**: Performance change from baseline
- **p-value**: Statistical significance (p < 0.05 indicates significant change)

## Regression Detection

### CLI Tool Regression Detection (relative-to-best and baseline)

Automatically detects regressions by comparing against fastest fixture:

```bash
cargo run --bin berry-bench-bin -- --all -r 5
```

**Output**:

```
Performance Analysis:
  workspaces.yarn.lock performance looks normal (1.0x vs fastest)
  minimal-berry.lock is 2.8x slower than workspaces.yarn.lock (potential regression)
```

**Thresholds**:

- **Warning**: >50% slower than fastest fixture
- **Critical**: >100% slower than fastest fixture

Additionally, compare against a stored baseline using normalized ms/KiB:

```bash
cargo run --bin berry-bench-bin -- --all --baseline .bench/baseline.json --threshold-ratio-ms-per-kib 0.05 --fail-on-regression
```

This exits with code 1 if any fixture regresses more than 5% in ms/KiB.

### Criterion Regression Detection

Automatically detects statistically significant performance changes:

```
change: [+22.753% +25.201% +27.608%] (p = 0.00 < 0.05)
Performance has regressed.
```

**Interpretation**:

- **p < 0.05**: Statistically significant change
- **Positive change**: Performance regression
- **Negative change**: Performance improvement

## Memory Analysis

### Heap Usage Tracking

Tracks both physical and virtual memory:

```bash
# Verbose memory output
cargo run --bin berry-bench-bin -- -f minimal-berry.lock -v
```

**Output**:

```
Heap usage: 20480 bytes (physical), 0 bytes (virtual)
```

**Interpretation**:

- **Physical memory**: Actual heap usage in bytes
- **Virtual memory**: Virtual memory allocation
- **Zero virtual memory**: Often indicates zero-allocation success

### Zero-Allocation Validation

Some fixtures show 0 bytes heap usage, validating zero-allocation claims:

```
yarn4-mixed-protocol.lock: 0 bytes heap usage
yarn4-patch.lock: 0 bytes heap usage
```

## Performance Optimization

### Development Workflow

1. **Establish baseline**:

   ```bash
   cargo run --bin berry-bench-bin -- --all -r 10
   ```

2. **Make changes**: Implement optimizations

3. **Test performance**:

   ```bash
   cargo run --bin berry-bench-bin -- --all -r 10
   ```

4. **Compare results**: Look for improvements or regressions

5. **Detailed analysis** (if needed):
   ```bash
   cargo bench --package berry-bench --bench parser_benchmarks
   ```

### Optimization

- Use `&str` instead of `String` during parsing
- Use `fold_many0` instead of `many0`
- Defer allocation until final data structures
- Parse everything in one pass

### Adding New Fixtures

1. Add fixture to `fixtures/` directory
2. Update benchmark lists in both CLI and Criterion benchmarks
3. Test parsing to ensure compatibility
4. Run benchmarks to establish baseline

#### Custom Benchmark Categories

```rust
// Add to crates/berry-bench/benches/parser_benchmarks.rs
fn benchmark_custom_category(c: &mut Criterion) {
    let mut group = c.benchmark_group("custom_category");

    // Your custom benchmarks here

    group.finish();
}
```

```bash
# Compare against baseline
cargo run --bin berry-bench-bin -- --all --format json > current-results.json
diff baseline-results.json current-results.json
```

### Debug Tools

```bash
# Debug parsing issues
RUST_LOG=debug cargo test

# Profile with flamegraph
cargo install flamegraph
cargo flamegraph --bench parser_benchmarks

# Memory profiling
cargo run --bin berry-bench-bin -- -f large-fixture.lock -v

# Detailed Criterion analysis
cargo bench --package berry-bench --bench parser_benchmarks -- --verbose
```

## Related

- [Criterion Documentation](https://docs.rs/criterion/) - Statistical benchmarking framework
- [Nom Documentation](https://docs.rs/nom/) - Parser combinator library
- [Memory Stats Documentation](https://docs.rs/memory-stats/) - Memory usage tracking
- [Core Parser](CORE_PARSER.md) - Parser implementation details
