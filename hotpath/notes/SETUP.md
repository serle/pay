# Hotpath Profiling Setup Complete

## Summary

A dedicated `hotpath/` folder has been created with profiling infrastructure using the [hotpath](https://crates.io/crates/hotpath) crate for clean, easy-to-read performance analysis.

## What's Included

### 1. **Profiling Binaries**

Two profiling targets have been created:

- `hotpath_single_threaded` - Profiles synchronous processing (1M transactions)
- `hotpath_multi_threaded` - Profiles concurrent processing (100 streams × 10K transactions)

### 2. **Configuration**

- Added `hotpath` as an optional dependency
- Created `profiling` feature flag
- Multi-threaded profile uses 8 worker threads (optimal from benchmarks)

### 3. **Documentation**

- **hotpath/README.md** - Comprehensive guide to using hotpath profiling
- **hotpath/SETUP.md** - This file
- **hotpath/.gitignore** - Excludes generated reports from git

## Quick Start

### Run Single-Threaded Profile

```bash
cargo run --release --bin hotpath_single_threaded --features profiling
```

**Expected output:**
- Function call counts
- Total time per function
- Average time per call
- Percentage of total execution time

**What this reveals:**
- Pure business logic costs (domain operations)
- Storage overhead (DashMap operations)
- Arithmetic costs (fixed-point operations)

### Run Multi-Threaded Profile

```bash
cargo run --release --bin hotpath_multi_threaded --features profiling
```

**Expected output:**
- Same metrics as single-threaded
- Plus: Concurrency overhead (Tokio, work-stealing, contention)

**What this reveals:**
- Thread coordination costs
- Lock contention in DashMap
- Tokio runtime overhead
- Difference from single-threaded (concurrency cost)

### Save Results for Analysis

```bash
# Save single-threaded results
cargo run --release --bin hotpath_single_threaded --features profiling > hotpath/single_threaded_report.txt

# Save multi-threaded results
cargo run --release --bin hotpath_multi_threaded --features profiling > hotpath/multi_threaded_report.txt
```

## What to Expect

Based on the benchmark results, the hotpath profile will likely show:

### Single-Threaded Hotpath

| Function Category | Expected % | What It Means |
|-------------------|------------|---------------|
| `process_transaction` | 40-50% | Main orchestration logic |
| DashMap operations | 20-30% | Storage lookups and updates |
| Domain operations | 15-25% | apply_deposit, apply_withdrawal, etc. |
| Fixed-point arithmetic | 5-10% | FixedPoint operations |
| Error handling | 5-10% | Validation and error propagation |

### Multi-Threaded Hotpath

| Function Category | Expected % | What It Means |
|-------------------|------------|---------------|
| DashMap operations | 30-40% | Increased due to contention |
| Tokio spawn/await | 10-20% | Task spawning and coordination |
| `process_transaction` | 30-40% | Reduced % due to overhead |
| Domain operations | 10-20% | Reduced % due to overhead |
| Work-stealing | 5-10% | Tokio scheduler overhead |

## Comparing Results

### Key Metrics to Compare

1. **Total Time**
   - Single-threaded: ~5s for 1M transactions
   - Multi-threaded: ~1.5s for 1M transactions (3x faster expected)

2. **DashMap Percentage**
   - Single-threaded: 20-30%
   - Multi-threaded: 30-40% (contention increases relative cost)

3. **Tokio Overhead**
   - Single-threaded: 0%
   - Multi-threaded: 10-20% (cost of concurrency)

### What Good Results Look Like

✅ **Single-Threaded:**
- No single function dominates (> 50%)
- Storage operations < 35%
- Error handling < 15%

✅ **Multi-Threaded:**
- Tokio overhead < 25%
- Lock contention visible but not dominating
- Speedup of 2-4x vs single-threaded

❌ **Red Flags:**
- Any function > 60% of time (algorithmic issue)
- Error handling > 30% (too many validations)
- Tokio overhead > 40% (thread count too high)

## Integration with Benchmarks

Use hotpath profiling to **understand** what benchmarks **measure**:

1. **Criterion benchmarks** show WHAT is slow:
   - "Processing 10K transactions takes 500µs"

2. **Hotpath profiling** shows WHY it's slow:
   - "40% of time is in DashMap lookups"
   - "25% of time is in apply_deposit validation"

3. **Optimize the hot paths** identified by hotpath

4. **Re-run Criterion** to validate improvement

## Troubleshooting

### "No profiling output"

**Problem:** Hotpath not instrumenting code

**Solution:** Ensure `--features profiling` flag is used

### "Binary not found"

**Problem:** Binary not compiled

**Solution:**
```bash
cargo build --release --bin hotpath_single_threaded --features profiling
cargo build --release --bin hotpath_multi_threaded --features profiling
```

### "Results don't match expectations"

**Problem:** Different workload than benchmarks

**Solution:**
- Check workload size matches (1M transactions)
- Ensure `--release` mode is used
- Verify thread count (8 for multi-threaded)

## Next Steps

1. **Run both profiles** to establish baseline
2. **Compare single vs multi-threaded** to understand concurrency cost
3. **Identify hot paths** (functions > 10% of time)
4. **Focus optimization** on highest-impact functions
5. **Re-profile** after changes to validate improvements
6. **Save results** to track performance over time

## Technical Details

### Hotpath Configuration

The `hotpath` crate is configured as an optional dependency:

```toml
[dependencies]
hotpath = { version = "0.5", optional = true }

[features]
default = []
profiling = ["hotpath"]
```

This allows:
- **Production builds** without profiling overhead (`cargo build --release`)
- **Profiling builds** with instrumentation (`cargo build --release --features profiling`)

### Instrumented Functions

Functions annotated with `#[hotpath::measure]`:
- `run_workload()` - Main profiling loop
- `process_deposit()` - Deposit transaction handling
- `process_withdrawal()` - Withdrawal transaction handling
- `process_dispute()` - Dispute transaction handling

Deeper instrumentation would add more overhead but provide finer-grained insight.

## References

- [Hotpath Crate Documentation](https://docs.rs/hotpath)
- [Hotpath GitHub](https://github.com/pawurb/hotpath)
- [hotpath/README.md](README.md) - Full profiling guide
- [BENCHMARK_RESULTS.md](../BENCHMARK_RESULTS.md) - Performance metrics
- [RUNTIME_ANALYSIS.md](../RUNTIME_ANALYSIS.md) - Thread scaling analysis
