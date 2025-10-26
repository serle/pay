# Automated Test Suite

This directory contains a comprehensive automated test suite that simulates the actual automated scoring environment for the payment transaction engine.

## Purpose

- **Validate submission**: Ensures the solution passes all expected automated tests
- **Regression testing**: Quickly verify changes don't break existing functionality
- **Documentation**: Provides clear examples of expected behavior
- **Confidence**: Demonstrates the solution works correctly across all scenarios

## Structure

```
auto_tester/
├── run_tests.sh          # Main test runner script
├── scenarios/            # Input CSV test files (14 test cases)
│   ├── 01_basic_deposits_withdrawals.csv
│   ├── 02_dispute_resolve.csv
│   ├── 03_dispute_chargeback.csv
│   ├── 04_insufficient_funds.csv
│   ├── 05_multiple_clients.csv
│   ├── 06_decimal_precision.csv
│   ├── 07_locked_account.csv
│   ├── 08_invalid_transactions.csv
│   ├── 09_whitespace.csv
│   ├── 10_dispute_partial_withdrawal.csv
│   ├── 11_empty.csv
│   ├── 12_client_mismatch.csv
│   ├── 13_multiple_disputes.csv
│   └── 14_large_amounts.csv
├── expected/             # Expected output CSV files
│   └── <corresponding test outputs>
└── README.md            # This file
```

## Running Tests

### Quick Run

```bash
./auto_tester/run_tests.sh
```

### From Project Root

```bash
cd /path/to/pay
./auto_tester/run_tests.sh
```

## Test Coverage

### Core Functionality (Tests 1-3)
- ✅ **01: Basic deposits and withdrawals** - Fundamental operations
- ✅ **02: Dispute → resolve** - Dispute workflow completion
- ✅ **03: Dispute → chargeback** - Dispute workflow with account locking

### Error Handling (Tests 4, 8)
- ✅ **04: Insufficient funds** - Withdrawals fail gracefully, total unchanged
- ✅ **08: Invalid transactions** - Invalid types ignored silently

### Multiple Clients (Test 5)
- ✅ **05: Multiple clients** - Concurrent account handling

### Precision & Formatting (Tests 6, 9, 14)
- ✅ **06: Decimal precision** - 1-4 decimal places accepted and output correctly
- ✅ **09: Whitespace** - CSV whitespace handling per spec
- ✅ **14: Large amounts** - High-value transactions

### Account States (Test 7)
- ✅ **07: Locked account** - All operations rejected after chargeback

### Complex Scenarios (Tests 10, 12, 13)
- ✅ **10: Dispute after withdrawal** - Partial fund disputes
- ✅ **12: Client mismatch** - Wrong client disputes ignored
- ✅ **13: Multiple disputes** - Concurrent dispute tracking

### Edge Cases (Test 11)
- ✅ **11: Empty CSV** - Handles empty input gracefully

## Test Runner Features

### Automated Comparison
- Builds the project in release mode
- Runs each scenario against the binary
- Compares actual output with expected output
- **Handles row ordering**: Normalizes CSVs before comparison (brief specifies "row ordering does not matter")

### Clear Reporting
- ✓ Green checkmarks for passing tests
- ✗ Red X for failing tests
- Summary statistics
- Diff output for failures

### Exit Codes
- **0**: All tests passed (ready for submission)
- **1**: One or more tests failed (fix before submission)

## Expected Output Format

All expected outputs follow the brief specification:

```csv
client,available,held,total,locked
<client_id>,<amount>,<amount>,<amount>,<true|false>
```

With:
- Exactly 4 decimal places for all amounts
- Values satisfy: `total = available + held`
- `locked` is `true` after chargeback, `false` otherwise
- Row ordering is non-deterministic (test runner handles this)

## Adding New Tests

1. Create input CSV in `scenarios/`:
   ```bash
   echo "type,client,tx,amount
   deposit,1,1,100.0" > scenarios/15_my_test.csv
   ```

2. Generate expected output:
   ```bash
   cargo run --release -- scenarios/15_my_test.csv > expected/15_my_test.csv
   ```

3. Verify manually that the output is correct

4. Run test suite:
   ```bash
   ./auto_tester/run_tests.sh
   ```

## Integration with CI/CD

This test suite can be integrated into CI/CD pipelines:

```yaml
# Example GitHub Actions workflow
- name: Run automated tests
  run: ./auto_tester/run_tests.sh
```

## Comparison with Manual Testing

**Advantages over manual testing:**
- ✅ Repeatable: Run anytime, always same results
- ✅ Fast: All 14 tests run in ~1 second
- ✅ Comprehensive: Covers all brief requirements
- ✅ Regression-proof: Catches breaking changes immediately
- ✅ Documentation: Tests serve as executable specifications

## Test Philosophy

These tests are designed to:

1. **Mirror automated scoring**: Uses same I/O as actual evaluation
2. **Cover all requirements**: Every brief requirement has test coverage
3. **Validate edge cases**: Tests boundary conditions and error paths
4. **Prove correctness**: Passing all tests demonstrates compliance
5. **Build confidence**: Reduces submission anxiety

## Success Criteria

When `run_tests.sh` shows:

```
╔════════════════════════════════════════════════════════════╗
║  ALL TESTS PASSED - Ready for submission! 🎉              ║
╚════════════════════════════════════════════════════════════╝
```

You can be confident the solution will pass automated scoring.

## Troubleshooting

### Test fails but output looks correct
- Check for trailing whitespace differences
- Verify decimal precision (exactly 4 places)
- Ensure CSV header matches exactly

### Binary not found
```bash
cargo build --release
```

### Permission denied
```bash
chmod +x auto_tester/run_tests.sh
```

## Maintenance

- Tests are deterministic and should not require updates
- If brief requirements change, update corresponding test cases
- Keep expected outputs in sync with actual behavior changes
