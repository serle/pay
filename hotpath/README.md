# Hotpath Profiling

This directory contains profiling scripts using the [hotpath](https://crates.io/crates/hotpath) crate to identify performance bottlenecks in the payment transaction engine.

**Hotpath** is a lightweight Rust profiler that shows exactly where your code spends time and allocates memory, with much cleaner output than raw `perf` or flamegraph tools.

## Available Profiles

All profiling binaries are located in the `hotpath/` folder to keep profiling infrastructure organized separately from the main codebase.

### 1. Single-Threaded Profile (`hotpath/single_threaded.rs`)

Profiles synchronous transaction processing without concurrency overhead.

**Purpose:** Identify bottlenecks in core business logic:
- Domain operations (deposits, withdrawals, disputes)
- Storage operations (DashMap lookups, updates)
- Fixed-point arithmetic overhead
- Transaction validation costs

**Run:**
```bash
cargo run --release --bin hotpath_single_threaded --features profiling
```

**Output:** Console report showing:
- Function call counts
- Total time per function
- Average time per call
- Percentage of total execution time

### 2. Multi-Threaded Profile (`hotpath/multi_threaded.rs`)

Profiles concurrent transaction processing with Tokio runtime.

**Purpose:** Identify concurrency-specific bottlenecks:
- Thread coordination overhead
- Lock contention in DashMap
- Tokio task spawning costs
- Work-stealing scheduler overhead

**Run:**
```bash
cargo run --release --bin hotpath_multi_threaded --features profiling
```

**Configuration:** Uses 8 worker threads (optimal from benchmarks) with 100 concurrent streams.

### 3. High Contention Profile (`hotpath/high_contention.rs`)

Profiles with Zipf distribution (80/20 access pattern).

**Purpose:** Test realistic hotspot scenarios:
- 20% of accounts receive 80% of traffic
- High contention on popular accounts
- Validates DashMap performance under skewed load

**Run:**
```bash
cargo run --release --bin hotpath_high_contention --features profiling
```

### 4. Workflow Stress Profile (`hotpath/workflow_stress.rs`)

Profiles heavy dispute/resolve/chargeback workflows.

**Purpose:** Stress test transaction store with lookups:
- 40% full dispute workflows
- Tests transaction store lookup performance
- Validates workflow operation efficiency

**Run:**
```bash
cargo run --release --bin hotpath_workflow_stress --features profiling
```

### 5. Store Intensive Profile (`hotpath/store_intensive.rs`)

Profiles with 60% transaction store operations.

**Purpose:** Isolate transaction store performance:
- 50%+ operations require store lookups
- Single-threaded to remove concurrency effects
- Determines if store is a bottleneck

**Run:**
```bash
cargo run --release --bin hotpath_store_intensive --features profiling
```

### 6. Sparse Account IDs Profile (`hotpath/sparse_accounts.rs`) ⭐ **Most Realistic**

Profiles with realistic sparse, non-sequential account IDs.

**Purpose:** Test with production-like account IDs:
- Non-sequential IDs (simulates UUIDs/large random numbers)
- Realistic hash distribution
- Proves impact of sparse IDs on performance

**Run:**
```bash
cargo run --release --bin hotpath_sparse_accounts --features profiling
```

**Why This Matters:** Sequential account IDs (1, 2, 3...) used in other tests are unrealistic and overstate performance by ~13%. This profile uses realistic sparse IDs and shows the actual expected performance in production.

## Understanding Hotpath Output

Hotpath generates a detailed report showing where time is spent:

```
Function Name                      | Calls    | Total Time | Avg Time  | % Total
-----------------------------------|----------|------------|-----------|--------
process_transaction                | 1000000  | 2.345s     | 2.3µs     | 45.2%
DashMap::entry                     | 1000000  | 1.123s     | 1.1µs     | 21.6%
apply_deposit                      | 600000   | 0.812s     | 1.4µs     | 15.6%
apply_withdrawal                   | 300000   | 0.456s     | 1.5µs     | 8.8%
...
```

### Key Metrics

- **Calls**: How many times the function was invoked
- **Total Time**: Cumulative time spent in the function (includes called functions)
- **Avg Time**: Average time per function call
- **% Total**: Percentage of total program execution time

### What to Look For

**Hot Paths (> 10% of time):**
- These are the primary candidates for optimization
- Focus on functions with both high call count AND high average time

**Unexpected Bottlenecks:**
- Functions that shouldn't be expensive but show high % total
- Often indicate algorithmic inefficiencies or unnecessary work

**Low-Hanging Fruit:**
- Functions with high call count but low average time
- May benefit from batching or caching

## Instrumenting Custom Code

To profile your own code, use hotpath macros:

```rust
#[hotpath::main]
fn main() {
    // Your code here - profiling automatically enabled
}

#[hotpath::measure]
fn my_function() {
    // This function will be profiled
}

fn helper() {
    hotpath::measure_block!("critical_section", {
        // Profile just this block
    });
}
```

## Comparing Single vs Multi-Threaded

Run both profiles and compare results:

**Expected Differences:**

| Aspect | Single-Threaded | Multi-Threaded |
|--------|-----------------|----------------|
| **Domain operations** | 40-50% | 30-40% (diluted by concurrency overhead) |
| **DashMap operations** | 20-30% | 30-40% (increased due to contention) |
| **Tokio overhead** | 0% | 10-20% (task spawning, work-stealing) |
| **Total time** | ~5s for 1M tx | ~1.5s for 1M tx (3x speedup expected) |

**Key Insights:**
- **Single-threaded:** Shows pure business logic costs
- **Multi-threaded:** Shows concurrency overhead and contention points

## Integration with Benchmarks

Hotpath results complement Criterion benchmarks:

- **Criterion:** Measures WHAT is slow (end-to-end time)
- **Hotpath:** Reveals WHY it's slow (which functions dominate)

Use together for maximum insight:
1. Run Criterion to identify slow scenarios
2. Run Hotpath to find bottlenecks within those scenarios
3. Optimize the hot paths
4. Re-run Criterion to validate improvements

## Tips for Effective Profiling

### 1. Use Release Mode
Always profile with `--release`:
```bash
cargo run --release --bin hotpath_single_threaded
```

Debug builds have 10-100x slower performance and different bottlenecks.

### 2. Sufficient Workload
Profile with enough work to generate stable results:
- Single-threaded: 1M transactions (current setting)
- Multi-threaded: 100 streams × 10K transactions (current setting)

### 3. Warm-Up Period
The profiling examples include warm-up to eliminate:
- JIT compilation effects (minimal in Rust)
- Cache priming costs
- Allocator initialization

### 4. Isolate Concerns
If results are unclear, create focused profiles:
```rust
#[hotpath::main]
fn main() {
    // Profile ONLY deposits
    for i in 0..1_000_000 {
        processor.process_transaction(Transaction::Deposit { ... });
    }
}
```

## Advanced Usage

### Custom Reporters

Hotpath supports custom output formats:

```rust
use hotpath::{MetricsProvider, Reporter};

struct MyReporter;

impl Reporter for MyReporter {
    fn report(&self, metrics: &dyn MetricsProvider) {
        // Custom output format
    }
}
```

### Filtering Results

Focus on specific modules or patterns:

```rust
#[hotpath::measure_all]
mod my_critical_module {
    // All functions in this module are profiled

    #[hotpath::skip]
    fn trivial_helper() {
        // Skip profiling this one
    }
}
```

## Troubleshooting

### "No profiling data collected"

**Cause:** Functions not annotated with `#[hotpath::measure]`

**Fix:** Use `#[hotpath::main]` and `#[hotpath::measure]` macros

### "Profiling overhead too high"

**Cause:** Too many small functions being profiled

**Fix:** Use `#[hotpath::skip]` on trivial functions

### "Results don't match benchmarks"

**Cause:** Different workload or compiler optimizations

**Fix:** Ensure both use `--release` and similar workload sizes

## Further Reading

- [Hotpath Crate Documentation](https://docs.rs/hotpath)
- [Hotpath GitHub Repository](https://github.com/pawurb/hotpath)
- [Performance Profiling in Rust](https://nnethercote.github.io/perf-book/)
- [The Rust Performance Book](https://nnethercote.github.io/perf-book/profiling.html)

## Output Location

Profiling reports are printed to stdout. To save for later analysis:

```bash
cargo run --release --bin hotpath_single_threaded --features profiling > hotpath/single_threaded_report.txt
cargo run --release --bin hotpath_multi_threaded --features profiling > hotpath/multi_threaded_report.txt
```

These reports can be checked into git to track performance changes over time.
