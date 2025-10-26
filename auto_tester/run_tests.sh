#!/bin/bash
#
# Automated Test Suite for Payment Transaction Engine
# Simulates automated scoring environment
#
# Usage: ./auto_tester/run_tests.sh
#

set -e

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Counters
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0

# Get script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

echo "╔════════════════════════════════════════════════════════════╗"
echo "║  Automated Test Suite - Payment Transaction Engine        ║"
echo "╚════════════════════════════════════════════════════════════╝"
echo ""

# Build the project
echo "Building project..."
cd "$PROJECT_ROOT"
if cargo build --release --quiet 2>&1 | grep -E "error" > /dev/null; then
    echo -e "${RED}✗ Build failed${NC}"
    exit 1
fi
echo -e "${GREEN}✓ Build successful${NC}"
echo ""

BINARY="$PROJECT_ROOT/target/release/pay"

if [ ! -f "$BINARY" ]; then
    echo -e "${RED}✗ Binary not found: $BINARY${NC}"
    exit 1
fi

# Function to normalize CSV for comparison
# Sorts rows (except header) and removes trailing whitespace
normalize_csv() {
    local file="$1"
    # Extract header
    head -n 1 "$file"
    # Sort remaining rows (if any)
    tail -n +2 "$file" | sort 2>/dev/null || true
}

# Function to run a single test
run_test() {
    local scenario_file="$1"
    local test_name="$(basename "$scenario_file" .csv)"
    local expected_file="$SCRIPT_DIR/expected/$test_name.csv"
    local actual_file="/tmp/pay_test_${test_name}_actual.csv"

    TOTAL_TESTS=$((TOTAL_TESTS + 1))

    # Run the test
    if ! "$BINARY" "$scenario_file" > "$actual_file" 2>/dev/null; then
        echo -e "${RED}✗ $test_name - Binary execution failed${NC}"
        FAILED_TESTS=$((FAILED_TESTS + 1))
        return 1
    fi

    # Normalize both files for comparison
    local expected_normalized="/tmp/pay_test_${test_name}_expected_norm.csv"
    local actual_normalized="/tmp/pay_test_${test_name}_actual_norm.csv"

    normalize_csv "$expected_file" > "$expected_normalized"
    normalize_csv "$actual_file" > "$actual_normalized"

    # Compare outputs
    if diff -q "$expected_normalized" "$actual_normalized" > /dev/null 2>&1; then
        echo -e "${GREEN}✓${NC} $test_name"
        PASSED_TESTS=$((PASSED_TESTS + 1))
        # Cleanup temp files on success
        rm -f "$actual_file" "$expected_normalized" "$actual_normalized"
        return 0
    else
        echo -e "${RED}✗${NC} $test_name"
        echo "  Expected:"
        cat "$expected_file" | head -5
        echo "  Actual:"
        cat "$actual_file" | head -5
        echo "  Run: diff $expected_file $actual_file"
        FAILED_TESTS=$((FAILED_TESTS + 1))
        return 1
    fi
}

# Run all tests
echo "Running test scenarios..."
echo "────────────────────────────────────────────────────────────"

for scenario in "$SCRIPT_DIR"/scenarios/*.csv; do
    run_test "$scenario"
done

echo "────────────────────────────────────────────────────────────"
echo ""

# Print summary
echo "Test Summary:"
echo "  Total:  $TOTAL_TESTS"
echo -e "  ${GREEN}Passed: $PASSED_TESTS${NC}"

if [ $FAILED_TESTS -gt 0 ]; then
    echo -e "  ${RED}Failed: $FAILED_TESTS${NC}"
    echo ""
    echo -e "${RED}╔════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${RED}║  TESTS FAILED                                             ║${NC}"
    echo -e "${RED}╚════════════════════════════════════════════════════════════╝${NC}"
    exit 1
else
    echo -e "  ${GREEN}Failed: 0${NC}"
    echo ""
    echo -e "${GREEN}╔════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${GREEN}║  ALL TESTS PASSED 🎉                                      ║${NC}"
    echo -e "${GREEN}╚════════════════════════════════════════════════════════════╝${NC}"
    exit 0
fi
