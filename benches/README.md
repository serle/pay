# Performance Benchmarking Guide

This document describes the comprehensive benchmarking infrastructure for the payment transaction engine, implemented using [Criterion.rs](https://github.com/bheisler/criterion.rs).

## Overview

The benchmarking suite validates the architectural claims made in the design, particularly:
1. **High throughput** transaction processing
2. **Concurrent scaling** to thousands of streams
3. **Lock-free** performance characteristics
4. **Efficient memory** usage

## Benchmark Categories

### 1. Transaction Processing (`transaction_processing.rs`)

**Purpose:** Establish single-threaded baseline performance

**Benchmarks:**
- `deposit_throughput` - Measures raw deposit processing speed (100, 1K, 10K transactions)
- `withdrawal_throughput` - Tests withdrawal processing with pre-funded accounts
- `dispute_workflow` - Full dispute → resolve cycles
- `chargeback_workflow` - Dispute → chargeback with account locking
- `mixed_workload` - Realistic 70% deposits, 20% withdrawals, 10% disputes
- `locked_account_overhead` - Cost of rejecting operations on locked accounts

**Key Metrics:**
- Transactions per second
- Operation latency
- Memory allocation patterns

**Expected Performance:**
- Single-threaded: 100K-500K transactions/sec
- Mixed workload: 50K-200K transactions/sec (due to validation overhead)

### 2. Storage Operations (`storage_operations.rs`)

**Purpose:** Validate DashMap-based storage performance

**Benchmarks:**
- `account_entry_cold` - First access to accounts (cache miss)
- `account_entry_hot` - Repeated access (cache hit)
- `account_update` - try_update() operation throughput
- `account_read` - Read-only access patterns
- `transaction_store_insert` - Transaction insertion speed
- `transaction_store_get` - Lookup performance
- `transaction_store_contains` - Existence check speed
- `mixed_account_ops` - 70% reads, 30% updates (realistic pattern)

**Key Metrics:**
- Operations per second
- Impact of account count (100, 1K, 10K, 100K)
- Cache behavior (cold vs hot)

**Expected Performance:**
- Account operations: 1M+ ops/sec (DashMap is very fast)
- Transaction store: 500K+ ops/sec for gets
- Scales sub-linearly with account count

### 3. Concurrent Streams (`concurrent_streams.rs`)

**Purpose:** Validate "thousands of concurrent TCP streams" claim

**Benchmarks:**
- `concurrent_streams_scaling` - Scale from 1 to 10,000 streams
  - Tests: 1, 10, 100, 1,000, 10,000 concurrent streams
  - Each stream processes 100 transactions
- `high_contention` - All streams access same account (worst case)
- `low_contention` - Disjoint accounts (best case parallelism)
- `error_policy_concurrent` - Impact of error handling under load
- `zipf_distribution` - Realistic access pattern (80/20 rule)

**Key Metrics:**
- Total system throughput (transactions/sec across all streams)
- Per-stream throughput
- Scaling efficiency (speedup vs number of streams)
- Contention impact

**Expected Performance:**
- Low contention: Near-linear scaling up to CPU core count
- High contention: Degraded but still functional
- 1,000 streams: Validates production readiness
- 10,000 streams: Validates architectural headroom

### 4. End-to-End (`end_to_end.rs`)

**Purpose:** Measure real-world CSV pipeline performance

**Benchmarks:**
- `csv_pipeline_sizes` - Small (1K), Medium (10K), Large (100K) datasets
- `csv_client_distributions` - 1, 100, 1K, 10K clients (contention testing)
- `csv_transaction_patterns`:
  - Deposit-heavy (90% deposits - onboarding scenario)
  - Balanced (50% deposits, 30% withdrawals, 10% disputes)
  - Withdrawal-heavy (60% withdrawals - trading scenario)
  - Dispute-heavy (30% disputes - stress test)
- `snapshot_generation` - CSV output performance (100, 1K, 10K, 100K accounts)
- `error_handling_overhead` - Cost of SkipErrors vs SilentSkip policies
- `parsing_vs_processing` - Identify bottlenecks (I/O vs compute)

**Key Metrics:**
- End-to-end throughput (transactions/sec)
- Wall clock time for datasets
- Memory high-water mark
- Parsing overhead vs processing overhead

**Expected Performance:**
- End-to-end: 50K-200K transactions/sec (I/O bound)
- Snapshot generation: Sub-second for 10K accounts
- Parsing overhead: ~20-30% of total time

## Running Benchmarks

### Run All Benchmarks
```bash
cargo bench
```

### Run Specific Benchmark Suite
```bash
cargo bench --bench transaction_processing
cargo bench --bench storage_operations
cargo bench --bench concurrent_streams
cargo bench --bench end_to_end
```

### Run Specific Benchmark Function
```bash
cargo bench --bench transaction_processing -- deposit_throughput
cargo bench --bench concurrent_streams -- scaling
```

### Save Baseline for Comparison
```bash
# Save current performance as baseline
cargo bench -- --save-baseline main

# Make changes to code...

# Compare against baseline
cargo bench -- --baseline main
```

## Interpreting Results

### Criterion Output

Criterion provides detailed statistics for each benchmark:

```
deposit_throughput/10000
                        time:   [18.234 ms 18.456 ms 18.702 ms]
                        thrpt:  [534.69 Kelem/s 541.94 Kelem/s 548.44 Kelem/s]
```

- **time**: Execution time (lower is better)
  - First value: Lower bound of 95% confidence interval
  - Second value: Point estimate
  - Third value: Upper bound of 95% confidence interval
- **thrpt**: Throughput (higher is better)
  - Kelem/s = thousands of elements (transactions) per second
  - Melem/s = millions of elements per second

### HTML Reports

Criterion generates detailed HTML reports in `target/criterion/`:

```bash
# Open in browser
open target/criterion/report/index.html
```

Reports include:
- Performance plots over time
- Probability density functions
- Regression analysis
- Outlier detection

### Performance Regression Detection

Criterion automatically detects statistically significant performance changes:

```
deposit_throughput/10000
                        time:   [18.234 ms 18.456 ms 18.702 ms]
                        change: [-5.2311% -3.8924% -2.6102%] (p = 0.00 < 0.05)
                        Performance has improved.
```

- **change**: Percentage change from baseline
- **p-value**: Statistical significance (< 0.05 = significant)

## Performance Targets

Based on the concurrent architecture with DashMap:

| Metric | Target | Rationale |
|--------|--------|-----------|
| Single-threaded throughput | 100K-500K tx/sec | Domain + storage overhead |
| Concurrent scaling (low contention) | Linear to core count | DashMap per-shard locking |
| Concurrent scaling (high contention) | 50-70% of low contention | Expected with single account |
| 1,000 concurrent streams | Handle smoothly | Production requirement |
| 10,000 concurrent streams | No crashes/deadlocks | Architectural headroom |
| End-to-end CSV (10K tx) | < 100ms | Interactive response |
| End-to-end CSV (1M tx) | < 10s | Batch processing |
| Snapshot (10K accounts) | < 1s | Non-blocking operation |
| Memory per account | < 1KB | Scalability |

## Regression Testing in CI

To integrate benchmarks into continuous integration:

### GitHub Actions Example

```yaml
name: Performance Regression
on: [pull_request]

jobs:
  benchmark:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: dtolnay/rust-toolchain@stable

      # Run benchmarks and save results
      - name: Run benchmarks
        run: |
          cargo bench --bench transaction_processing -- --save-baseline pr
          cargo bench --bench concurrent_streams -- --save-baseline pr

      # Compare with main branch baseline
      - name: Check for regressions
        run: |
          git fetch origin main
          git checkout origin/main
          cargo bench -- --baseline pr
```

## Profiling Integration

### CPU Profiling with flamegraph

```bash
# Install flamegraph
cargo install flamegraph

# Profile a specific benchmark
cargo flamegraph --bench concurrent_streams -- --profile-time=10
```

### Memory Profiling with valgrind

```bash
# Install valgrind (Linux only)
sudo apt-get install valgrind

# Profile memory usage
valgrind --tool=massif cargo bench --bench end_to_end
ms_print massif.out.*
```

## Benchmark Maintenance

### When to Update Benchmarks

1. **New features**: Add benchmarks for new transaction types or operations
2. **Architectural changes**: Update if storage layer changes
3. **Performance optimizations**: Verify improvements with before/after baselines
4. **Bug fixes**: Ensure fixes don't degrade performance

### Adding New Benchmarks

1. Create function in appropriate benchmark file:
```rust
fn bench_new_feature(c: &mut Criterion) {
    c.bench_function("new_feature", |b| {
        b.iter_batched(
            || setup_state(),
            |state| perform_operation(state),
            BatchSize::SmallInput,
        )
    });
}
```

2. Add to criterion_group macro:
```rust
criterion_group!(
    benches,
    bench_existing,
    bench_new_feature,  // Add here
);
```

3. Run and verify:
```bash
cargo bench --bench your_benchmark
```

## Troubleshooting

### Benchmarks are noisy/inconsistent

- Close other applications
- Disable CPU frequency scaling: `sudo cpupower frequency-set --governor performance`
- Run multiple samples: `cargo bench -- --sample-size 100`

### Out of memory errors

- Reduce dataset sizes in benchmarks
- Use `BatchSize::SmallInput` or `BatchSize::NumIterations(n)`
- Profile memory usage to find leaks

### Benchmarks take too long

- Reduce sample size: `--sample-size 10`
- Run specific benchmarks instead of all
- Use `--quick` mode: `cargo bench -- --quick`

## References

- [Criterion.rs Documentation](https://bheisler.github.io/criterion.rs/book/)
- [Statistical Analysis Explained](https://bheisler.github.io/criterion.rs/book/analysis.html)
- [DashMap Performance Characteristics](https://docs.rs/dashmap/)
- [Tokio Benchmarking Guide](https://tokio.rs/tokio/topics/benchmarking)
